/// Why the peer refused an outbound resource.
///
/// A refusal arrives as a `RESOURCE_RCL` packet carrying only the resource
/// hash (`RNS/Resource.py:155-165`, `Resource.reject`), so the sender never
/// learns the peer's motive from the wire; the variants beyond
/// [`PeerCancelled`](Self::PeerCancelled) exist for refusals the sender can
/// classify on its own.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// The peer answered the advertisement with a receiver cancel.
    PeerCancelled,
    /// The resource exceeded a cap the sender knew about locally. No code
    /// path produces this variant in 0.15.0; it is reserved.
    Oversize {
        /// The cap in bytes that the resource exceeded.
        limit: usize,
    },
    /// No code path produces this variant in 0.15.0; it is reserved for an
    /// unclassified refusal.
    Unknown,
}

/// How an outbound resource stopped short of being proven.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundFailure {
    /// The advertisement was never dispatched, so the peer never heard of
    /// the transfer.
    NotSent,
    /// The retry budget ran out while the resource was advertised,
    /// transferring or awaiting its proof.
    RetriesExhausted,
    /// A later segment of a split resource could not be built, or the sender
    /// failed while serving the peer's part requests: a part or hash-map
    /// packet that would not build, or a request out of sequence. The second
    /// path is open to a single-segment send as well.
    Segmenting,
    /// The link went away under the transfer. This is also what an outcome
    /// resolves to when the transport is dropped before reaching a verdict.
    LinkClosed,
}

/// The terminal verdict of one outbound resource.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundResult {
    /// The peer proved the final segment.
    Complete,
    /// The transfer stopped without a proof; see [`OutboundFailure`].
    Failed(OutboundFailure),
    /// The peer refused the transfer; see [`RejectReason`].
    Rejected(RejectReason),
    /// The local side cancelled the transfer.
    Cancelled,
}

/// Resolves once with the verdict of the outbound resource it was issued for.
///
/// Handed out by [`ResourceManager::track_prepared_with_outcome`]. If the
/// manager is dropped before reaching a verdict the future resolves to
/// `Failed(`[`OutboundFailure::LinkClosed`]`)` rather than hanging.
#[must_use = "the outcome resolves once; dropping it loses the verdict"]
pub struct OutboundOutcome {
    rx: tokio::sync::oneshot::Receiver<OutboundResult>,
}

impl std::fmt::Debug for OutboundOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutboundOutcome").finish_non_exhaustive()
    }
}

impl std::future::Future for OutboundOutcome {
    type Output = OutboundResult;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.rx)
            .poll(cx)
            .map(|verdict| verdict.unwrap_or(OutboundResult::Failed(OutboundFailure::LinkClosed)))
    }
}

impl ResourceManager {
    /// [`track_prepared`] plus a future that resolves with the transfer's
    /// verdict.
    ///
    /// The outcome is keyed by the original hash, so a split resource
    /// resolves once, when its final segment concludes, not per segment.
    ///
    /// [`track_prepared`]: ResourceManager::track_prepared
    pub fn track_prepared_with_outcome(
        &mut self,
        prepared: PreparedSend,
    ) -> (Hash, Packet, OutboundOutcome) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.outbound_outcomes.insert(prepared.first.original_hash, tx);
        let (resource_hash, packet) = self.track_prepared(prepared);
        (resource_hash, packet, OutboundOutcome { rx })
    }

    /// The one exit for an outbound resource: queues the terminal
    /// [`ResourceEvent`] and resolves the outcome registered for it, if any.
    /// Every terminal path goes through here so the two cannot disagree.
    pub(crate) fn conclude_outbound(
        &mut self,
        original_hash: Hash,
        link_id: AddressHash,
        result: OutboundResult,
    ) {
        let kind = match &result {
            OutboundResult::Complete => ResourceEventKind::OutboundComplete,
            OutboundResult::Failed(_) => ResourceEventKind::OutboundFailed,
            OutboundResult::Rejected(reason) => {
                ResourceEventKind::OutboundRejected { reason: reason.clone() }
            }
            OutboundResult::Cancelled => ResourceEventKind::OutboundCancelled,
        };
        self.events.push(ResourceEvent { hash: original_hash, link_id, kind });
        if let Some(tx) = self.outbound_outcomes.remove(&original_hash) {
            // The receiver may already be gone; nobody is left to tell.
            let _ = tx.send(result);
        }
    }
}
