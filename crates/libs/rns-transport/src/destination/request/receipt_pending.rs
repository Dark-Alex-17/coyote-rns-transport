use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::watch;

use super::receipt::{
    RequestError, RequestReceipt, RequestResponseData, RequestStatus, ResponseArrival,
};
use super::RequestId;
use crate::hash::Hash;
use crate::resource::ResourceProgress;

#[derive(Default)]
pub(super) struct Bookkeeping {
    pub(super) progress: f32,
    pub(super) response_size: Option<usize>,
    pub(super) response_transfer_size: Option<usize>,
    pub(super) concluded_at: Option<Instant>,
}

/// What every clone of a [`RequestReceipt`] shares with the link's pending
/// entry. The outcome cell is written before the status watch is moved to
/// `Ready` or `Failed`, so a waiter woken by the watch always finds it set.
pub(super) struct ReceiptState {
    pub(super) request_id: RequestId,
    pub(super) started_at: Instant,
    pub(super) timeout: Duration,
    pub(super) outcome: OnceLock<Result<RequestResponseData, RequestError>>,
    bookkeeping: Mutex<Bookkeeping>,
}

impl ReceiptState {
    pub(super) fn bookkeeping(&self) -> std::sync::MutexGuard<'_, Bookkeeping> {
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
        RequestReceipt::new(self.status_tx.subscribe(), self.state.clone())
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

    /// Part of the response Resource arrived (`RNS/Link.py:1429`).
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
