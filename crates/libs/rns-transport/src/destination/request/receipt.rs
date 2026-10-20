use core::fmt;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::receipt_pending::ReceiptState;
use super::{FrameDecodeError, RequestId, ResponseFrame};

/// `Link.TRAFFIC_TIMEOUT_FACTOR` (`RNS/Link.py:82`).
const TRAFFIC_TIMEOUT_FACTOR: u32 = 6;

/// `RNS.Resource.RESPONSE_MAX_GRACE_TIME * 1.125` (`RNS/Resource.py:118`,
/// `RNS/Link.py:491`).
pub(crate) const RESPONSE_GRACE: Duration = Duration::from_millis(11_250);

/// Options for [`Transport::request`](crate::transport::Transport::request).
///
/// `timeout` defaults to `rtt * 6 + 11.25 s`
/// (`RNS/Link.py:491`: `rtt * traffic_timeout_factor +
/// RESPONSE_MAX_GRACE_TIME * 1.125`). `max_response_size` is the
/// `max_response_size` argument of `RNS.Link.request`: a response larger
/// than it is refused and the receipt fails with
/// [`RequestError::Oversize`].
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    timeout: Option<Duration>,
    max_response_size: Option<usize>,
}

impl RequestOptions {
    /// Fails the receipt with [`RequestError::Timeout`] once `timeout` has
    /// passed without a response, instead of the link-RTT default. The
    /// clock runs from the send for a request that fits one packet and from
    /// `Delivered` for one sent as a Resource (`RNS/Link.py:1331-1334`,
    /// `:1370-1373`), and stops while a response Resource is being received
    /// (`:1389`).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Refuses a response larger than `max_response_size` bytes, the check
    /// Python runs on a response Resource's advertisement
    /// (`RNS/Link.py:1048-1049`) and, through `handle_response(...,
    /// check_size=True)` (`:1017`), on a response packet (`:862-863`).
    pub fn with_max_response_size(mut self, max_response_size: usize) -> Self {
        self.max_response_size = Some(max_response_size);
        self
    }

    /// The response size limit, if one was set.
    pub(crate) fn max_response_size(&self) -> Option<usize> {
        self.max_response_size
    }

    /// The timeout in force for a link whose round-trip time is `rtt`.
    pub(crate) fn timeout_for_rtt(&self, rtt: Duration) -> Duration {
        self.timeout.unwrap_or(rtt * TRAFFIC_TIMEOUT_FACTOR + RESPONSE_GRACE)
    }
}

/// Where a request stands, as `RNS.RequestReceipt.status` reports it
/// (`RNS/Link.py:1318-1322`: `FAILED`, `SENT`, `DELIVERED`, `RECEIVING`,
/// `READY`).
///
/// A request sent as one packet goes `Sent -> Ready`, or `Sent ->
/// Receiving -> Ready` when the response is a Resource; nothing proves a
/// `REQUEST` packet, so `Delivered` is never reached that way
/// (`RNS/Link.py:1421-1427`, `:1445-1450` mark only the packet receipt). A
/// request sent as a Resource reaches `Delivered` when that Resource
/// completes (`RNS/Link.py:1372`). `Failed` ends any of them.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestStatus {
    /// `RequestReceipt.SENT` (`RNS/Link.py:1319`).
    Sent,
    /// `RequestReceipt.DELIVERED` (`RNS/Link.py:1320`).
    Delivered,
    /// `RequestReceipt.RECEIVING` (`RNS/Link.py:1321`).
    Receiving,
    /// `RequestReceipt.READY` (`RNS/Link.py:1322`).
    Ready,
    /// `RequestReceipt.FAILED` (`RNS/Link.py:1318`).
    Failed,
}

impl RequestStatus {
    pub(super) fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

/// Why a request failed. Python has the single `RequestReceipt.FAILED`
/// status (`RNS/Link.py:1318-1322`), reached from `request_timed_out`
/// (`:1397-1405`), `response_rejected` (`:1407-1411`) and
/// `request_resource_concluded` (`:1377-1381`); the variants here split that
/// status by cause.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// No response within the request timeout: counted from the send for a
    /// request packet, from `Delivered` for a request Resource. Python's
    /// `request_timed_out` (`RNS/Link.py:1397-1405`) only fails a receipt
    /// that is `DELIVERED` (`:1398`), which the receipt of a request packet
    /// never is, so it leaves that receipt pending; here it fails.
    #[error("no response before the request timeout")]
    Timeout,
    /// The link closed while the request was pending. Fork-only: Python's
    /// receipt has no link-closed arm; its `FAILED` covers every cause
    /// (`RNS/Link.py:1318-1322`).
    #[error("the link closed before a response arrived")]
    LinkClosed,
    /// The responder rejected the request Resource. Python fails the receipt
    /// from `request_resource_concluded` (`RNS/Link.py:1377-1381`); its
    /// `response_rejected` (`:1407-1411`) is the oversize-response path.
    #[error("the responder rejected the request resource")]
    Rejected,
    /// The response is larger than the `max_response_size` asked for. For a
    /// response packet (`RNS/Link.py:1017`, `:876`) Python's guard
    /// (`:1408`) only fails a receipt that is `DELIVERED`, which the receipt
    /// of a request packet never is, so it leaves that receipt pending; here
    /// it fails.
    #[error("the response exceeded the requested maximum size")]
    Oversize,
    /// The response is not a `[request_id, body]` envelope, the envelope of
    /// a response Resource names another request, or the metadata is not
    /// msgpack. Fork-only: Python indexes the envelope unchecked
    /// (`RNS/Link.py:1014-1015`) and an exception there is logged and
    /// dropped (`:1020`).
    #[error("the response could not be decoded")]
    Decode,
    /// The request Resource could not be sent: its retries ran out, its
    /// send failed, it was cancelled locally through
    /// `Transport::cancel_resource`, or the entry was displaced by a request
    /// under the same id. Fork-only: Python folds every request Resource
    /// failure into the one `request_resource_concluded` arm
    /// (`RNS/Link.py:1377-1381`).
    #[error("the request could not be sent")]
    Send,
    /// The response Resource failed in transfer before it completed: a part
    /// failure, receiver retry exhaustion or a remote cancel. Python
    /// (`RNS/Link.py:1435`) cancels the Resource when the receipt has
    /// failed; this is the other direction, which Python surfaces only
    /// through the Resource's own failure callback.
    #[error("the response resource failed before it completed")]
    Receive,
}

/// A decoded response.
///
/// A response packet, and a response Resource without metadata, carry the
/// `[request_id, body]` envelope; the body is the handler's return value in
/// its own msgpack type (`RNS/Link.py:907-911`). A response Resource with
/// metadata is a file response: its data is the raw file bytes, not an
/// envelope, and the metadata travels beside it (`RNS/Link.py:899-903`).
///
/// A cheap handle: clones, and every [`RequestReceipt::response`] call,
/// share one body.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestResponseData(Arc<ResponseBody>);

#[derive(Debug, Clone, PartialEq)]
enum ResponseBody {
    Value(rmpv::Value),
    File { data: Vec<u8>, metadata: rmpv::Value },
}

impl RequestResponseData {
    /// The envelope body, `unpacked_response[1]` (`RNS/Link.py:1015`,
    /// `:911`); `None` for a file response.
    pub fn value(&self) -> Option<&rmpv::Value> {
        match &*self.0 {
            ResponseBody::Value(value) => Some(value),
            ResponseBody::File { .. } => None,
        }
    }

    /// The envelope body (`RNS/Link.py:1015`, `:911`), moved out of the last
    /// handle or cloned out of a shared one; `None` for a file response.
    pub fn into_value(self) -> Option<rmpv::Value> {
        if let ResponseBody::File { .. } = &*self.0 {
            return None;
        }
        match Arc::try_unwrap(self.0).unwrap_or_else(|shared| (*shared).clone()) {
            ResponseBody::Value(value) => Some(value),
            ResponseBody::File { .. } => None,
        }
    }

    /// The raw bytes of a file response, the `resource.data` of a Resource
    /// with metadata (`RNS/Link.py:903`); `None` for an envelope.
    pub fn data(&self) -> Option<&[u8]> {
        match &*self.0 {
            ResponseBody::File { data, .. } => Some(data),
            ResponseBody::Value(_) => None,
        }
    }

    /// The decoded metadata of a file response, `RequestReceipt.metadata`
    /// (`RNS/Link.py:903`, `:1441`); `None` for an envelope.
    pub fn metadata(&self) -> Option<&rmpv::Value> {
        match &*self.0 {
            ResponseBody::File { metadata, .. } => Some(metadata),
            ResponseBody::Value(_) => None,
        }
    }
}

/// A response as it reaches the link, before it is decoded.
pub(crate) enum ResponseArrival<'a> {
    /// The decrypted `[request_id, body]` envelope of a `RESPONSE` packet
    /// and the packed length of its body, which the receipt reports as both
    /// response sizes. Python's figure (`RNS/Link.py:1016`) is this length
    /// less two.
    Packet { envelope: &'a [u8], response_size: usize },
    /// An assembled response Resource: the whole envelope when `metadata`
    /// is `None`, raw file bytes beside packed metadata otherwise
    /// (`RNS/Link.py:897-912`).
    Resource { data: &'a [u8], metadata: Option<&'a [u8]> },
}

/// Why a response did not decode. The receipt reports only
/// [`RequestError::Decode`]; the link logs the cause. `MetadataRead` and
/// `MetadataTrailing` are defence in depth behind the walker.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ResponseDecodeFailure {
    /// The envelope is not a `[request_id, body]` frame.
    #[error("envelope: {0}")]
    Frame(FrameDecodeError),
    /// The envelope of a response Resource names a request other than the
    /// one its advertisement (`q`) answered. Python reads the id out of the
    /// envelope and keys `handle_response` on it (`RNS/Link.py:908-912`);
    /// here the advertisement picked the receipt and the envelope has to
    /// agree with it.
    #[error("envelope answers request {actual}, the receipt is {expected}")]
    RequestIdMismatch { expected: RequestId, actual: RequestId },
    /// The file metadata is not one well-formed msgpack value.
    #[error("metadata: {0}")]
    Metadata(FrameDecodeError),
    /// The walker accepted the metadata but `rmpv` did not.
    #[error("metadata: {0}")]
    MetadataRead(rmpv::decode::Error),
    /// `rmpv` stopped before the end of the metadata the walker accepted.
    #[error("metadata: bytes follow the value")]
    MetadataTrailing,
}

impl ResponseArrival<'_> {
    /// Decodes the response to the request `request_id`. A packet envelope
    /// is what the link found the receipt by, so only the envelope of a
    /// response Resource is checked against the id.
    pub(super) fn decode(
        self,
        request_id: RequestId,
    ) -> Result<RequestResponseData, ResponseDecodeFailure> {
        let body = match self {
            Self::Packet { envelope, .. } => {
                let frame =
                    ResponseFrame::decode(envelope).map_err(ResponseDecodeFailure::Frame)?;
                ResponseBody::Value(frame.into_body())
            }
            Self::Resource { data: envelope, metadata: None } => {
                let frame =
                    ResponseFrame::decode(envelope).map_err(ResponseDecodeFailure::Frame)?;
                if frame.request_id() != request_id {
                    return Err(ResponseDecodeFailure::RequestIdMismatch {
                        expected: request_id,
                        actual: frame.request_id(),
                    });
                }
                ResponseBody::Value(frame.into_body())
            }
            Self::Resource { data, metadata: Some(packed) } => {
                // The same allocation-free walk the frames get (module doc of
                // `request`), so metadata is held to the nesting rule too.
                super::frame::validate(packed).map_err(ResponseDecodeFailure::Metadata)?;
                let mut cursor = Cursor::new(packed);
                let metadata = rmpv::decode::read_value(&mut cursor)
                    .map_err(ResponseDecodeFailure::MetadataRead)?;
                // The walker already saw exactly one value; this guards the
                // decode against the walker and rmpv disagreeing on where it
                // ends.
                if cursor.position() != packed.len() as u64 {
                    return Err(ResponseDecodeFailure::MetadataTrailing);
                }
                ResponseBody::File { data: data.to_vec(), metadata }
            }
        };
        Ok(RequestResponseData(Arc::new(body)))
    }
}

/// The requester's handle on one request: `RNS.RequestReceipt`
/// (`RNS/Link.py:1311-1500`) without callbacks. Every clone observes the
/// same request; [`response`](Self::response) resolves on any of them once
/// the request is `Ready` or `Failed`. Dropping every clone does not cancel
/// the request: the responder still serves it and the response still
/// reaches the link's `LinkEvent::Data(Response)` and `resource_events()`
/// broadcasts, unless the receipt's `max_response_size` refuses it: an
/// over-limit packet Response is not posted as `LinkEvent::Data`, and a
/// response Resource that assembles over it has its `Complete` withheld.
#[derive(Clone)]
pub struct RequestReceipt {
    status_rx: watch::Receiver<RequestStatus>,
    state: Arc<ReceiptState>,
}

impl RequestReceipt {
    pub(super) fn new(status_rx: watch::Receiver<RequestStatus>, state: Arc<ReceiptState>) -> Self {
        Self { status_rx, state }
    }

    /// The id the responder knows this request by: `RequestReceipt.request_id`
    /// (`RNS/Link.py:1341`), the packet's truncated hash or the request
    /// Resource's `request_id` (`:1332`, `:1337`).
    pub fn request_id(&self) -> RequestId {
        self.state.request_id
    }

    /// Where the request stands now: `RequestReceipt.status`
    /// (`RNS/Link.py:1318-1322`, `:1348`). A receipt whose link went away
    /// without concluding it reads `Failed`, with
    /// [`RequestError::LinkClosed`] as the outcome
    /// [`response`](Self::response) returns.
    pub fn status(&self) -> RequestStatus {
        match self.outcome() {
            Some(Ok(_)) => RequestStatus::Ready,
            Some(Err(_)) => RequestStatus::Failed,
            None => *self.status_rx.borrow(),
        }
    }

    /// The outcome, concluding `LinkClosed` first when the link dropped the
    /// entry without concluding it.
    fn outcome(&self) -> Option<&Result<RequestResponseData, RequestError>> {
        if self.status_rx.has_changed().is_err() {
            return Some(self.state.conclude(Err(RequestError::LinkClosed)));
        }
        self.state.outcome()
    }

    /// Share of the response received so far, `0.0` to `1.0`
    /// (`RequestReceipt.progress`, `RNS/Link.py:1429`, `:1439`), counted
    /// across every segment of a split response. A packet response jumps
    /// straight to `1.0`.
    pub fn progress(&self) -> f32 {
        self.state.bookkeeping().progress
    }

    /// The response size: `d` of the first advertisement of a response
    /// Resource, the packed body length of a response packet
    /// (`RNS/Link.py:1057-1058`, `:1016`). `None` until a response arrives.
    pub fn response_size(&self) -> Option<usize> {
        self.state.bookkeeping().response_size
    }

    /// The bytes on the wire: `t` summed over the segments of a response
    /// Resource, the same figure as [`response_size`](Self::response_size)
    /// for a packet (`RNS/Link.py:1059-1061`, `:1016`). `None` until a
    /// response arrives.
    pub fn response_transfer_size(&self) -> Option<usize> {
        self.state.bookkeeping().response_transfer_size
    }

    /// When the response clock started (`RequestReceipt.started_at`): the
    /// send for a request packet (`RNS/Link.py:1331-1334`); for a request
    /// Resource `None` until it completes or a response is advertised,
    /// whichever comes first (`:1370-1371`, `:1062-1063`).
    pub fn started_at(&self) -> Option<Instant> {
        self.state.bookkeeping().started_at
    }

    /// When the request reached `Ready` or `Failed`. Python keeps two clocks,
    /// `concluded_at` for a failure (`RNS/Link.py:1380`, `:1400`, `:1410`)
    /// and `response_concluded_at` for a response (`:1443`); this is both. A
    /// receipt whose link went away without concluding it is concluded here.
    pub fn concluded_at(&self) -> Option<Instant> {
        self.outcome();
        self.state.bookkeeping().concluded_at
    }

    /// A receiver that reports every status change from now on. Fork-only:
    /// Python registers `response_callback`, `failed_callback` and
    /// `progress_callback` instead (`RNS/Link.py:1360-1363`). The status
    /// current at the call is read with `borrow_and_update`, not reported
    /// as a change. A `watch` keeps only the latest value, so quick
    /// successive changes may be seen as one. Once the request concludes,
    /// or the link goes away without concluding it, the sender is dropped:
    /// `changed()` returns an error after the last change rather than a
    /// further one, and `borrow()` keeps the last status. For a link that
    /// went away, [`status`](Self::status) and [`response`](Self::response)
    /// conclude the receipt `LinkClosed`.
    pub fn subscribe(&self) -> watch::Receiver<RequestStatus> {
        let mut status_rx = self.status_rx.clone();
        status_rx.mark_unchanged();
        status_rx
    }

    /// Waits until the request is `Ready` or `Failed` and returns the
    /// response or the failure. Fork-only: the awaitable twin of
    /// `RequestReceipt.get_response()` (`RNS/Link.py:1478-1483`) and the
    /// `response_callback` (`:1456-1458`). A link that disappears without
    /// concluding the request concludes it here as
    /// [`RequestError::LinkClosed`], so [`status`](Self::status) and
    /// [`concluded_at`](Self::concluded_at) agree with what this returned.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn response(&self) -> Result<RequestResponseData, RequestError> {
        let mut status_rx = self.status_rx.clone();
        loop {
            let terminal = status_rx.borrow_and_update().is_terminal();
            if terminal || status_rx.changed().await.is_err() {
                break;
            }
        }
        self.state.conclude(Err(RequestError::LinkClosed)).clone()
    }

    #[cfg(test)]
    pub(crate) fn timeout(&self) -> Duration {
        self.state.timeout
    }
}

impl fmt::Debug for RequestReceipt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestReceipt")
            .field("request_id", &self.request_id())
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}
