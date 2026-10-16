use core::fmt;
use std::io::Cursor;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::{RequestId, ResponseFrame};
use crate::hash::Hash;
use crate::resource::ResourceProgress;

/// `Link.TRAFFIC_TIMEOUT_FACTOR` (`RNS/Link.py:82`).
const TRAFFIC_TIMEOUT_FACTOR: u32 = 6;

/// `RNS.Resource.RESPONSE_MAX_GRACE_TIME * 1.125` (`RNS/Resource.py:118`,
/// `RNS/Link.py:491`).
const RESPONSE_GRACE: Duration = Duration::from_millis(11_250);

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
    /// passed without a response, instead of the link-RTT default.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Refuses a response larger than `max_response_size` bytes.
    pub fn with_max_response_size(mut self, max_response_size: usize) -> Self {
        self.max_response_size = Some(max_response_size);
        self
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    pub fn max_response_size(&self) -> Option<usize> {
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
    Sent,
    Delivered,
    Receiving,
    Ready,
    Failed,
}

impl RequestStatus {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

/// Why a request failed.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RequestError {
    /// No response within the request timeout.
    #[error("no response before the request timeout")]
    Timeout,
    /// The link closed while the request was pending.
    #[error("the link closed before a response arrived")]
    LinkClosed,
    /// The responder rejected the request Resource.
    #[error("the responder rejected the request resource")]
    Rejected,
    /// The response is larger than the `max_response_size` asked for.
    #[error("the response exceeded the requested maximum size")]
    Oversize,
    /// The response is not a `[request_id, body]` envelope, or its metadata
    /// is not msgpack.
    #[error("the response could not be decoded")]
    Decode,
    /// The request Resource could not be sent.
    #[error("the request could not be sent")]
    Send,
}

/// A decoded response.
///
/// A response packet, and a response Resource without metadata, carry the
/// `[request_id, body]` envelope; the body is the handler's return value in
/// its own msgpack type (`RNS/Link.py:907-911`). A response Resource with
/// metadata is a file response: its data is the raw file bytes, not an
/// envelope, and the metadata travels beside it (`RNS/Link.py:899-903`).
#[derive(Debug, Clone, PartialEq)]
pub struct RequestResponseData(ResponseBody);

#[derive(Debug, Clone, PartialEq)]
enum ResponseBody {
    Value(rmpv::Value),
    File { data: Vec<u8>, metadata: rmpv::Value },
}

impl RequestResponseData {
    /// The envelope body; `None` for a file response.
    pub fn value(&self) -> Option<&rmpv::Value> {
        match &self.0 {
            ResponseBody::Value(value) => Some(value),
            ResponseBody::File { .. } => None,
        }
    }

    /// The envelope body, moved out; `None` for a file response.
    pub fn into_value(self) -> Option<rmpv::Value> {
        match self.0 {
            ResponseBody::Value(value) => Some(value),
            ResponseBody::File { .. } => None,
        }
    }

    /// The raw bytes of a file response; `None` for an envelope.
    pub fn data(&self) -> Option<&[u8]> {
        match &self.0 {
            ResponseBody::File { data, .. } => Some(data),
            ResponseBody::Value(_) => None,
        }
    }

    /// The decoded metadata of a file response; `None` for an envelope.
    pub fn metadata(&self) -> Option<&rmpv::Value> {
        match &self.0 {
            ResponseBody::File { metadata, .. } => Some(metadata),
            ResponseBody::Value(_) => None,
        }
    }
}

/// A response as it reaches the link, before it is decoded.
pub(crate) enum ResponseArrival<'a> {
    /// The decrypted `[request_id, body]` envelope of a `RESPONSE` packet.
    Packet(&'a [u8]),
    /// An assembled response Resource: the whole envelope when `metadata`
    /// is `None`, raw file bytes beside packed metadata otherwise
    /// (`RNS/Link.py:896-912`).
    Resource { data: &'a [u8], metadata: Option<&'a [u8]> },
}

impl ResponseArrival<'_> {
    fn decode(self) -> Result<RequestResponseData, RequestError> {
        let body = match self {
            Self::Packet(envelope) | Self::Resource { data: envelope, metadata: None } => {
                let frame = ResponseFrame::decode(envelope).map_err(|_| RequestError::Decode)?;
                ResponseBody::Value(frame.into_body())
            }
            Self::Resource { data, metadata: Some(packed) } => {
                let mut cursor = Cursor::new(packed);
                let metadata =
                    rmpv::decode::read_value(&mut cursor).map_err(|_| RequestError::Decode)?;
                if cursor.position() != packed.len() as u64 {
                    return Err(RequestError::Decode);
                }
                ResponseBody::File { data: data.to_vec(), metadata }
            }
        };
        Ok(RequestResponseData(body))
    }
}

#[derive(Default)]
struct Bookkeeping {
    progress: f32,
    response_size: Option<usize>,
    response_transfer_size: Option<usize>,
    concluded_at: Option<Instant>,
}

/// What every clone of a [`RequestReceipt`] shares with the link's pending
/// entry. The outcome cell is written before the status watch is moved to
/// `Ready` or `Failed`, so a waiter woken by the watch always finds it set.
struct ReceiptState {
    request_id: RequestId,
    started_at: Instant,
    timeout: Duration,
    outcome: OnceLock<Result<RequestResponseData, RequestError>>,
    bookkeeping: Mutex<Bookkeeping>,
}

impl ReceiptState {
    fn bookkeeping(&self) -> std::sync::MutexGuard<'_, Bookkeeping> {
        self.bookkeeping.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The link's side of one outstanding request: `RNS.RequestReceipt` as the
/// entry of `Link.pending_requests` (`RNS/Link.py:1365`). Owns the status
/// sender; dropping it without concluding tells every receipt the link is
/// gone.
pub(crate) struct PendingRequest {
    status_tx: watch::Sender<RequestStatus>,
    state: Arc<ReceiptState>,
    request_resource_hash: Option<Hash>,
    response_resource_hash: Option<Hash>,
}

impl PendingRequest {
    /// A request just sent. `request_resource_hash` is the hash of the
    /// request Resource when the request left as one, so its sender-side
    /// events can be matched back to this entry.
    pub(crate) fn new(
        request_id: RequestId,
        timeout: Duration,
        request_resource_hash: Option<Hash>,
    ) -> Self {
        let (status_tx, _) = watch::channel(RequestStatus::Sent);
        Self {
            status_tx,
            state: Arc::new(ReceiptState {
                request_id,
                started_at: Instant::now(),
                timeout,
                outcome: OnceLock::new(),
                bookkeeping: Mutex::new(Bookkeeping::default()),
            }),
            request_resource_hash,
            response_resource_hash: None,
        }
    }

    pub(crate) fn receipt(&self) -> RequestReceipt {
        RequestReceipt { status_rx: self.status_tx.subscribe(), state: self.state.clone() }
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.state.started_at + self.state.timeout
    }

    pub(crate) fn status(&self) -> RequestStatus {
        *self.status_tx.borrow()
    }

    pub(crate) fn request_resource_hash(&self) -> Option<Hash> {
        self.request_resource_hash
    }

    pub(crate) fn response_resource_hash(&self) -> Option<Hash> {
        self.response_resource_hash
    }

    /// The request Resource completed (`RNS/Link.py:1372`).
    pub(crate) fn mark_delivered(&self) {
        if self.status() == RequestStatus::Sent {
            self.status_tx.send_replace(RequestStatus::Delivered);
        }
    }

    /// The response Resource was advertised: `d` and `t` of the
    /// advertisement become the response sizes (`RNS/Link.py:870-873`) and
    /// the receipt starts receiving (`RNS/Link.py:1420`).
    pub(crate) fn mark_receiving(
        &mut self,
        response_size: u64,
        response_transfer_size: u64,
        response_hash: Hash,
    ) {
        if self.status().is_terminal() {
            return;
        }
        self.response_resource_hash = Some(response_hash);
        {
            let mut bookkeeping = self.state.bookkeeping();
            bookkeeping.response_size = usize::try_from(response_size).ok();
            bookkeeping.response_transfer_size = usize::try_from(response_transfer_size).ok();
        }
        self.status_tx.send_replace(RequestStatus::Receiving);
    }

    /// Part of the response Resource arrived (`RNS/Link.py:1430`).
    pub(crate) fn note_progress(&self, progress: &ResourceProgress) {
        if progress.total_bytes == 0 {
            return;
        }
        self.state.bookkeeping().progress =
            (progress.received_bytes as f64 / progress.total_bytes as f64) as f32;
    }

    /// Decodes the response and concludes the request with it.
    pub(crate) fn resolve(self, arrival: ResponseArrival<'_>) {
        let outcome = arrival.decode();
        if outcome.is_ok() {
            self.state.bookkeeping().progress = 1.0;
        }
        self.conclude(outcome);
    }

    pub(crate) fn fail(self, error: RequestError) {
        self.conclude(Err(error));
    }

    fn conclude(self, outcome: Result<RequestResponseData, RequestError>) {
        let status = if outcome.is_ok() { RequestStatus::Ready } else { RequestStatus::Failed };
        self.state.bookkeeping().concluded_at = Some(Instant::now());
        // Set the cell first: a receipt woken by the status change reads it
        // without any further synchronisation.
        self.state.outcome.get_or_init(|| outcome);
        self.status_tx.send_replace(status);
    }
}

/// The requester's handle on one request: `RNS.RequestReceipt`
/// (`RNS/Link.py:1330-1452`) without callbacks. Every clone observes the
/// same request; [`response`](Self::response) resolves on any of them once
/// the request is `Ready` or `Failed`.
#[derive(Clone)]
pub struct RequestReceipt {
    status_rx: watch::Receiver<RequestStatus>,
    state: Arc<ReceiptState>,
}

impl RequestReceipt {
    pub fn request_id(&self) -> RequestId {
        self.state.request_id
    }

    pub fn status(&self) -> RequestStatus {
        *self.status_rx.borrow()
    }

    /// Share of the response received so far, `0.0` to `1.0`
    /// (`RequestReceipt.progress`, `RNS/Link.py:1431`). A packet response
    /// jumps straight to `1.0`.
    pub fn progress(&self) -> f32 {
        self.state.bookkeeping().progress
    }

    /// The advertised data size of a response Resource; `None` until the
    /// advertisement arrives and for a packet response.
    pub fn response_size(&self) -> Option<usize> {
        self.state.bookkeeping().response_size
    }

    /// The advertised transfer size of a response Resource, after
    /// compression; `None` as for [`response_size`](Self::response_size).
    pub fn response_transfer_size(&self) -> Option<usize> {
        self.state.bookkeeping().response_transfer_size
    }

    pub fn started_at(&self) -> Instant {
        self.state.started_at
    }

    /// When the request reached `Ready` or `Failed`.
    pub fn concluded_at(&self) -> Option<Instant> {
        self.state.bookkeeping().concluded_at
    }

    /// A receiver that reports every status change from now on. A `watch`
    /// keeps only the latest value, so quick successive changes may be seen
    /// as one.
    pub fn subscribe(&self) -> watch::Receiver<RequestStatus> {
        self.status_rx.clone()
    }

    /// Waits until the request is `Ready` or `Failed` and returns the
    /// response or the failure. A link that disappears without concluding
    /// the request reads as [`RequestError::LinkClosed`].
    pub async fn response(&self) -> Result<RequestResponseData, RequestError> {
        let mut status_rx = self.status_rx.clone();
        loop {
            let terminal = status_rx.borrow_and_update().is_terminal();
            if terminal || status_rx.changed().await.is_err() {
                break;
            }
        }
        self.state.outcome.get().cloned().unwrap_or(Err(RequestError::LinkClosed))
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
