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
    /// The peer answered the advertisement with a receiver cancel, the
    /// `RESOURCE_RCL` that `Resource.reject` sends (`RNS/Resource.py:155-165`).
    PeerCancelled,
    /// The resource exceeded a cap the sender knew about locally. Fork-only,
    /// reserved: no code path produces this variant yet.
    Oversize {
        /// The cap in bytes that the resource exceeded.
        limit: usize,
    },
    /// Fork-only, reserved for an unclassified refusal: no code path produces
    /// this variant yet.
    Unknown,
}

/// How an outbound resource stopped short of being proven. Fork-only:
/// Python's sender has the one `Resource.FAILED` status
/// (`RNS/Resource.py:151`, set at `:1102`) for every one of these.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundFailure {
    /// Fork-only: the advertisement was never dispatched, so the peer never
    /// heard of the transfer.
    NotSent,
    /// Fork-only: the retry budget ran out while the resource was advertised,
    /// transferring or awaiting its proof. Python's watchdog calls `cancel()`
    /// when `retries_left` hits zero while advertised (`RNS/Resource.py:588-590`)
    /// or awaiting the proof (`:659-661`), which sets `Resource.FAILED`
    /// (`:1102`); while transferring it waits a single RTT-scaled `max_wait`
    /// with no retries (`:643-650`), a divergence the crate README registers.
    RetriesExhausted,
    /// Fork-only: a later segment of a split resource could not be built, or
    /// the sender failed while serving the peer's part requests: a part or
    /// hash-map packet that would not build, or a request out of sequence.
    /// The second path is open to a single-segment send as well.
    Segmenting,
    /// Fork-only: the link went away under the transfer. This is also what an
    /// outcome resolves to when the transport is dropped before reaching a
    /// verdict.
    LinkClosed,
}

/// The terminal verdict of one outbound resource. Fork-only: Python reports
/// a sender's fate only through the Resource's status
/// (`RNS/Resource.py:144-153`) and its concluded callback (`:805-806`,
/// `:1118-1121`, `:1130-1134`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboundResult {
    /// The peer proved the final segment: `Resource.COMPLETE`
    /// (`RNS/Resource.py:150`).
    Complete,
    /// The transfer stopped without a proof, `Resource.FAILED`
    /// (`RNS/Resource.py:151`); see [`OutboundFailure`].
    Failed(OutboundFailure),
    /// The peer refused the transfer; see [`RejectReason`]. `Resource.REJECTED`
    /// (`RNS/Resource.py:153`), set in `_rejected` (`:1128`).
    Rejected(RejectReason),
    /// The local side cancelled the transfer. Fork-only: Python's `cancel()`
    /// sets `Resource.FAILED` (`RNS/Resource.py:1101-1102`).
    Cancelled,
}

/// Resolves once with the verdict of the outbound resource it was issued for.
///
/// Handed out by [`ResourceManager::track_prepared_with_outcome`]. If the
/// manager is dropped before reaching a verdict the future resolves to
/// `Failed(`[`OutboundFailure::LinkClosed`]`)` rather than hanging.
///
/// Fork-only: Python has no awaitable verdict, only the Resource callback
/// (`RNS/Resource.py:805-806`, `:1118-1121`, `:1130-1134`).
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
    /// Fork-only: Python's sender registers a callback on the Resource
    /// instead (`RNS/Resource.py:388`, `:1139`).
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
            if let Err(verdict) = tx.send(result) {
                log::debug!(
                    "resource outbound: outcome receiver dropped before the verdict hash={} link={} verdict={:?}",
                    original_hash,
                    link_id,
                    verdict
                );
            }
        }
    }
}
