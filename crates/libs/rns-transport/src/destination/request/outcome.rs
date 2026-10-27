use std::sync::{Arc, Mutex};

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
}

/// Resolves once with the [`ResponseOutcome`] of the request it was issued
/// for.
///
/// Handed out by [`InboundRequest::track_outcome`]. `Transport::stop` or
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
