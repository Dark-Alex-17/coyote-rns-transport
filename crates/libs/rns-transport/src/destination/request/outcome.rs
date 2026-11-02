use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::oneshot;

use crate::resource::OutboundResult;

/// What became of the response a handler returned. Fork-only: Python's
/// `handle_request` sends the response and keeps nothing
/// (`RNS/Link.py:846-852`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseOutcome {
    /// Fork-only: the response left as one `RESPONSE` packet that reached
    /// at least one interface. No proof exists for a packet response (the
    /// README section "Delivery proofs on a link"), so this is dispatched,
    /// not delivered.
    Packet,
    /// Fork-only: the response left as a resource, either a
    /// `RequestResponse::Resource` or a `Value`/`Bytes` body that outgrew
    /// the link MDU, and this is that transfer's verdict.
    Resource(OutboundResult),
    /// Fork-only: nothing left. The handler returned `None`, the link could
    /// no longer exchange data, the send failed before dispatch (the packet
    /// would not build, the link has no ingress interface, the metadata does
    /// not pack, or the packet reached no interface), the handler panicked
    /// after tracking, or `Transport::stop` or `cancel` aborted the
    /// dispatcher task before it resolved: every path that drops the sender.
    NotSent,
    /// Fork-only: the response exceeded the destination's
    /// `max_response_size` and nothing was sent. `len` is the size the
    /// dispatcher measured, the packed `[request_id, body]` frame for a
    /// `Value` or `Bytes` response, or, for a `RequestResponse::Resource`,
    /// the size its advertisement carries: the uncompressed data together
    /// with the packed metadata and its length prefix (a metadata-less
    /// resource measures as its data alone); `max` is the cap in force.
    TooLarge {
        /// Fork-only: the measured response size in bytes.
        len: usize,
        /// Fork-only: the destination's `max_response_size` at dispatch.
        max: usize,
    },
}

/// Resolves once with the [`ResponseOutcome`] of the request it was issued
/// for.
///
/// Handed out by [`InboundRequest::track_outcome`] and
/// [`ResponseOutcomeTracker::outcome`]. `Transport::stop` or
/// `cancel` drops the dispatcher's `JoinSet`, which aborts the response task
/// and drops the sender, so the future resolves
/// [`ResponseOutcome::NotSent`] rather than hanging; a bare
/// [`OutboundOutcome`] on the same path would resolve
/// `Failed(LinkClosed)`.
///
/// Fork-only: Python has no awaitable verdict for a response
/// (`RNS/Link.py:846-852`).
///
/// [`InboundRequest::track_outcome`]: super::InboundRequest::track_outcome
/// [`OutboundOutcome`]: crate::resource::OutboundOutcome
#[must_use = "the outcome resolves once; dropping it loses the verdict"]
pub struct ResponseOutcomeReceiver {
    rx: oneshot::Receiver<ResponseOutcome>,
}

impl std::fmt::Debug for ResponseOutcomeReceiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponseOutcomeReceiver").finish_non_exhaustive()
    }
}

impl std::future::Future for ResponseOutcomeReceiver {
    type Output = ResponseOutcome;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.rx)
            .poll(cx)
            .map(|outcome| outcome.unwrap_or(ResponseOutcome::NotSent))
    }
}

/// The outcome slot of an [`InboundRequest`] once
/// [`into_parts`](super::InboundRequest::into_parts) has split the request
/// data away from it. Lets a handler consume the data by value first and
/// decide afterwards whether to watch the response.
///
/// [`outcome`](Self::outcome) follows the same timing rule as
/// [`InboundRequest::track_outcome`]: arm it inside the handler, before the
/// handler returns. The dispatcher takes the sender as the handler returns,
/// so a tracker armed after that is never taken: the sender stays in the
/// slot and drops when the dispatcher releases it (or at once, if the
/// dispatcher has already finished), and the receiver resolves
/// [`ResponseOutcome::NotSent`] without ever learning whether the response
/// went out. Dropping a tracker that was never armed leaves the slot as it
/// was: with no earlier `track_outcome`, the dispatcher logs nothing and
/// resolves nothing, and there is no "outcome receiver dropped" line for a
/// handler that chose not to watch.
///
/// Fork-only: Python's `handle_request` keeps nothing of the response it
/// sends (`RNS/Link.py:846-852`).
///
/// [`InboundRequest`]: super::InboundRequest
/// [`InboundRequest::track_outcome`]: super::InboundRequest::track_outcome
#[must_use = "arm it with outcome() before the handler returns, or drop it on purpose"]
pub struct ResponseOutcomeTracker {
    slot: OutcomeSlot,
}

impl std::fmt::Debug for ResponseOutcomeTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResponseOutcomeTracker").finish_non_exhaustive()
    }
}

impl ResponseOutcomeTracker {
    pub(crate) fn from_slot(slot: OutcomeSlot) -> Self {
        Self { slot }
    }

    /// A future that resolves with what became of the response, under the
    /// timing rule in the type docs. Replaces any sender an earlier
    /// `track_outcome` installed on the same request; that receiver
    /// resolves [`ResponseOutcome::NotSent`].
    ///
    /// Fork-only: Python's `handle_request` keeps nothing of the response it
    /// sends (`RNS/Link.py:846-852`).
    pub fn outcome(self) -> ResponseOutcomeReceiver {
        let (tx, rx) = oneshot::channel();
        *self.slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(tx);
        ResponseOutcomeReceiver::new(rx)
    }
}

/// Where a handler leaves the sender for the dispatcher to resolve once the
/// response has gone out. Shared between the `InboundRequest` the handler
/// consumes and the dispatcher task that outlives it.
pub(crate) type OutcomeSlot = Arc<Mutex<Option<oneshot::Sender<ResponseOutcome>>>>;

pub(crate) fn empty_outcome_slot() -> OutcomeSlot {
    Arc::new(Mutex::new(None))
}

impl ResponseOutcomeReceiver {
    pub(crate) fn new(rx: oneshot::Receiver<ResponseOutcome>) -> Self {
        Self { rx }
    }
}
