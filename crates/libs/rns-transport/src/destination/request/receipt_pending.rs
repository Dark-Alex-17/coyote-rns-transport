use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::receipt::{
    RequestError, RequestReceipt, RequestResponseData, RequestStatus, ResponseArrival,
    ResponseDecodeFailure,
};
use super::RequestId;
use crate::hash::Hash;
use crate::resource::{ResourceAdvertisement, ResourceProgress};

#[derive(Default)]
pub(super) struct Bookkeeping {
    pub(super) progress: f32,
    pub(super) response_size: Option<usize>,
    pub(super) response_transfer_size: Option<usize>,
    pub(super) started_at: Option<Instant>,
    pub(super) concluded_at: Option<Instant>,
}

/// What every clone of a [`RequestReceipt`] shares with the link's pending
/// entry. The outcome cell is written before the status watch is moved to
/// `Ready` or `Failed`, so a waiter woken by the watch always finds it set.
pub(super) struct ReceiptState {
    pub(super) request_id: RequestId,
    pub(super) timeout: Duration,
    outcome: OnceLock<Result<RequestResponseData, RequestError>>,
    bookkeeping: Mutex<Bookkeeping>,
}

impl ReceiptState {
    pub(super) fn bookkeeping(&self) -> std::sync::MutexGuard<'_, Bookkeeping> {
        self.bookkeeping.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn outcome(&self) -> Option<&Result<RequestResponseData, RequestError>> {
        self.outcome.get()
    }

    /// Writes the outcome and `concluded_at` once; a later call leaves both
    /// as they are and returns the first outcome.
    pub(super) fn conclude(
        &self,
        outcome: Result<RequestResponseData, RequestError>,
    ) -> &Result<RequestResponseData, RequestError> {
        self.outcome.get_or_init(|| {
            self.bookkeeping().concluded_at = Some(Instant::now());
            outcome
        })
    }
}

/// The response Resource being received, as its latest segment advertised
/// it. `Progress` events and a receiver cancel name the segment in flight,
/// `InboundFailed` names the whole Resource.
struct InboundResponse {
    segment_hash: Hash,
    original_hash: Hash,
    segment_index: u32,
    total_segments: u32,
}

/// The response Resource a failed receipt was still receiving, under both
/// hashes the ResourceManager knows it by: the segment in flight, which its
/// receiver and a receiver cancel name, and the whole Resource, which the
/// partial assembly of a split response is keyed by. Between one segment's
/// completion and the next segment's admitted advertisement only the
/// assembly exists, so a cancel needs both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InboundResponseHashes {
    pub(crate) segment: Hash,
    pub(crate) original: Hash,
}

/// The link's side of one outstanding request: `RNS.RequestReceipt` as the
/// entry of `Link.pending_requests` (`RNS/Link.py:1365`). Owns the status
/// sender; dropping it without concluding tells every receipt the link is
/// gone.
pub(crate) struct PendingRequest {
    status_tx: watch::Sender<RequestStatus>,
    state: Arc<ReceiptState>,
    request_resource_hash: Option<Hash>,
    response: Option<InboundResponse>,
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
        // The packet form starts its clock when it is sent
        // (`RNS/Link.py:1331-1334`); the resource form only once the request
        // Resource has completed (`:1370-1371`), however long that takes.
        let started_at = request_resource_hash.is_none().then(Instant::now);
        Self {
            status_tx,
            state: Arc::new(ReceiptState {
                request_id,
                timeout,
                outcome: OnceLock::new(),
                bookkeeping: Mutex::new(Bookkeeping { started_at, ..Bookkeeping::default() }),
            }),
            request_resource_hash,
            response: None,
        }
    }

    pub(crate) fn receipt(&self) -> RequestReceipt {
        RequestReceipt::new(self.status_tx.subscribe(), self.state.clone())
    }

    /// When the request timeout fails this entry; `None` while no clock
    /// runs: before the resource form is `Delivered` (`RNS/Link.py:1373`
    /// starts the clock there), while a response Resource is being received
    /// (`:1389` keeps the clock only while `DELIVERED`; that Resource's own
    /// failure fails the receipt instead), or when the timeout does not fit
    /// an `Instant`.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        if self.status() == RequestStatus::Receiving {
            return None;
        }
        self.state.bookkeeping().started_at?.checked_add(self.state.timeout)
    }

    pub(crate) fn status(&self) -> RequestStatus {
        *self.status_tx.borrow()
    }

    pub(crate) fn request_resource_hash(&self) -> Option<Hash> {
        self.request_resource_hash
    }

    /// The response Resource being received, under both its hashes.
    pub(crate) fn inbound_response(&self) -> Option<InboundResponseHashes> {
        self.response.as_ref().map(|response| InboundResponseHashes {
            segment: response.segment_hash,
            original: response.original_hash,
        })
    }

    /// The segment of the response Resource in flight, as `Progress` events
    /// and a receiver cancel name it.
    pub(crate) fn response_segment_hash(&self) -> Option<Hash> {
        self.response.as_ref().map(|response| response.segment_hash)
    }

    /// The response Resource as a whole, as `InboundFailed` names it.
    pub(crate) fn response_original_hash(&self) -> Option<Hash> {
        self.response.as_ref().map(|response| response.original_hash)
    }

    /// The request Resource completed (`RNS/Link.py:1370-1373`): the receipt
    /// is `Delivered` and the response clock starts. Python (`:1372`) sets
    /// `DELIVERED` unconditionally; here a receipt already `Receiving` keeps
    /// its state and its clock.
    pub(crate) fn mark_delivered(&self) {
        if self.status() == RequestStatus::Sent {
            self.state.bookkeeping().started_at.get_or_insert_with(Instant::now);
            self.status_tx.send_replace(RequestStatus::Delivered);
        }
    }

    /// A segment of the response Resource was advertised and admitted, that
    /// is, the ResourceManager built a receiver for it (`RNS/Link.py:1055-1064`
    /// touches the receipt only when `Resource.accept` returned one): `d` of
    /// the first advertisement is the response size, `t` adds up over the
    /// segments, the clock starts if the request Resource has not started it,
    /// and the receipt is `Receiving` (`:1420`).
    pub(crate) fn mark_receiving(&mut self, advertisement: &ResourceAdvertisement) {
        if self.status().is_terminal() {
            return;
        }
        // The sender re-advertises a segment it got no part request for
        // (`RNS/Resource.py:594-598`); a repeat of the segment in flight
        // changes nothing, `t` included (`RNS/Resource.py:238-240` hands
        // `RNS/Link.py:1056` no Resource for it).
        if self
            .response
            .as_ref()
            .is_some_and(|response| response.segment_hash == advertisement.hash)
        {
            return;
        }
        self.response = Some(InboundResponse {
            segment_hash: advertisement.hash,
            original_hash: advertisement.original_hash,
            segment_index: advertisement.segment_index,
            total_segments: advertisement.total_segments,
        });
        {
            let mut bookkeeping = self.state.bookkeeping();
            bookkeeping
                .response_size
                .get_or_insert(usize::try_from(advertisement.data_size).unwrap_or(usize::MAX));
            let transfer_size = bookkeeping.response_transfer_size.get_or_insert(0);
            *transfer_size = transfer_size
                .saturating_add(usize::try_from(advertisement.transfer_size).unwrap_or(usize::MAX));
            bookkeeping.started_at.get_or_insert_with(Instant::now);
        }
        self.status_tx.send_replace(RequestStatus::Receiving);
    }

    /// Part of the current segment arrived (`RNS/Link.py:1429`). Progress
    /// counts the segments already assembled as whole shares, as
    /// `Resource.get_progress` does across a split transfer
    /// (`RNS/Resource.py:1145-1200`).
    pub(crate) fn note_progress(&self, progress: &ResourceProgress) {
        let Some(response) = &self.response else {
            return;
        };
        if progress.total_bytes == 0 || response.total_segments == 0 {
            return;
        }
        let segment = progress.received_bytes as f64 / progress.total_bytes as f64;
        let assembled = f64::from(response.segment_index.saturating_sub(1));
        self.state.bookkeeping().progress =
            ((assembled + segment) / f64::from(response.total_segments)).min(1.0) as f32;
    }

    /// Decodes the response and concludes the request with it: `Ready`, or
    /// `Failed(Decode)` with the cause handed back for the caller to log. A
    /// response Resource whose envelope names another request is a decode
    /// failure too: its advertisement chose this entry, the envelope must
    /// agree (`RNS/Link.py:908-912` keys on the envelope alone).
    #[must_use = "the cause is for the caller to log"]
    pub(crate) fn resolve(self, arrival: ResponseArrival<'_>) -> Option<ResponseDecodeFailure> {
        if let ResponseArrival::Packet { response_size, .. } = &arrival {
            // A packet response advertises nothing; Python reports the packed
            // body as both sizes (`RNS/Link.py:1016-1017`, `:868-871`).
            let mut bookkeeping = self.state.bookkeeping();
            bookkeeping.response_size = Some(*response_size);
            bookkeeping.response_transfer_size = Some(*response_size);
        }
        match arrival.decode(self.state.request_id) {
            Ok(response) => {
                self.state.bookkeeping().progress = 1.0;
                self.conclude(Ok(response));
                None
            }
            Err(cause) => {
                self.conclude(Err(RequestError::Decode));
                Some(cause)
            }
        }
    }

    /// Concludes `Failed(error)`. Returns the response Resource that was
    /// still being received: the caller cancels that transfer
    /// (`RNS/Link.py:1435`), this entry only forgets it.
    pub(crate) fn fail(self, error: RequestError) -> Option<InboundResponseHashes> {
        let receiving = self.inbound_response();
        self.conclude(Err(error));
        receiving
    }

    fn conclude(self, outcome: Result<RequestResponseData, RequestError>) {
        let status = if outcome.is_ok() { RequestStatus::Ready } else { RequestStatus::Failed };
        // Set the cell first: a receipt woken by the status change reads it
        // without any further synchronisation.
        self.state.conclude(outcome);
        self.status_tx.send_replace(status);
    }
}
