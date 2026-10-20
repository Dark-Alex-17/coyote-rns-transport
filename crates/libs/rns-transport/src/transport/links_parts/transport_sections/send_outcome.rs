use crate::resource::OutboundOutcome;

impl Transport {
    /// [`send_resource_with_compression`](Self::send_resource_with_compression)
    /// plus an [`OutboundOutcome`] for the transfer.
    ///
    /// The outcome resolves once, with the same verdict the resource events
    /// report: [`OutboundResult::Complete`] when the peer proves the final
    /// segment, `Failed(`[`OutboundFailure`]`)` when the retry budget runs
    /// out, a later segment cannot be built or the link goes away,
    /// `Rejected(`[`RejectReason::PeerCancelled`]`)` when the peer answers
    /// the advertisement with a receiver cancel (`RNS/Resource.py:155-165`)
    /// and [`OutboundResult::Cancelled`] after
    /// [`cancel_resource`](Self::cancel_resource).
    ///
    /// Dropping the outcome cancels nothing: the transfer runs on and
    /// [`resource_events`](Self::resource_events) still carries its terminal
    /// event. `Err(ConnectionError)` means the advertisement never left, so
    /// there is no transfer to follow.
    ///
    /// [`OutboundResult::Complete`]: crate::resource::OutboundResult::Complete
    /// [`OutboundResult::Cancelled`]: crate::resource::OutboundResult::Cancelled
    /// [`OutboundFailure`]: crate::resource::OutboundFailure
    /// [`RejectReason::PeerCancelled`]: crate::resource::RejectReason::PeerCancelled
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn send_resource_with_outcome(
        &self,
        link_id: &AddressHash,
        data: Vec<u8>,
        metadata: Option<Vec<u8>>,
        auto_compress: bool,
    ) -> Result<(Hash, OutboundOutcome), RnsError> {
        self.send_resource_outcome_inner(link_id, data, metadata, |_| {}, auto_compress).await
    }

    /// [`send_request_resource_with_max_response_size`](Self::send_request_resource_with_max_response_size)
    /// plus an [`OutboundOutcome`] for the request Resource itself.
    ///
    /// The verdicts are those of
    /// [`send_resource_with_outcome`](Self::send_resource_with_outcome); a
    /// destination whose `max_request_size` the advertisement exceeds
    /// refuses it with a receiver cancel (`RNS/Link.py:1036-1042`), which
    /// resolves the outcome to `Rejected(PeerCancelled)`. The outcome says
    /// nothing about the response the peer may send back; that arrives
    /// through [`resource_events`](Self::resource_events) as before.
    ///
    /// Dropping the outcome cancels nothing; the transfer runs on and its
    /// terminal event is still published.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn send_request_resource_with_outcome(
        &self,
        link_id: &AddressHash,
        request_id: Vec<u8>,
        data: Vec<u8>,
        metadata: Option<Vec<u8>>,
        max_response_size: Option<usize>,
    ) -> Result<(Hash, OutboundOutcome), RnsError> {
        self.send_request_resource_outcome_inner(
            link_id,
            request_id,
            data,
            metadata,
            max_response_size,
        )
        .await
    }

    /// [`send_response_resource_with_compression`](Self::send_response_resource_with_compression)
    /// plus an [`OutboundOutcome`] for the response Resource.
    ///
    /// The verdicts are those of
    /// [`send_resource_with_outcome`](Self::send_resource_with_outcome). A
    /// requester whose `max_response_size` the advertisement exceeds refuses
    /// it with a receiver cancel (`RNS/Link.py:1044-1051`), resolving the
    /// outcome to `Rejected(PeerCancelled)`; one that no longer expects a
    /// response under `request_id` drops the advertisement without answering
    /// (`RNS/Link.py:1046-1047`), so that outcome ends in `Failed(RetriesExhausted)`.
    ///
    /// Dropping the outcome cancels nothing; the transfer runs on and its
    /// terminal event is still published.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn send_response_resource_with_outcome(
        &self,
        link_id: &AddressHash,
        request_id: Vec<u8>,
        data: Vec<u8>,
        metadata: Option<Vec<u8>>,
        auto_compress: bool,
    ) -> Result<(Hash, OutboundOutcome), RnsError> {
        super::request_dispatch::send_response_resource(
            &self.handler,
            link_id,
            request_id,
            data,
            metadata,
            auto_compress,
        )
        .await
    }
}
