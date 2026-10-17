// The requester's pending-request table, `Link.pending_requests` in
// `RNS/Link.py`. Every method is synchronous and runs under the link lock;
// the packet and resource paths that receive a response call in from there,
// and the watchdog expires timeouts from `check_watchdog`. A method that
// fails an entry hands back the response Resource it was receiving, if any:
// the entry cannot reach the ResourceManager, so the caller cancels that
// transfer (`RNS/Link.py:1435`).

impl Link {
    /// Registers a request just sent and returns the requester's handle. An
    /// entry already pending under the same id is failed with
    /// `RequestError::Send`, and the second value is the response Resource
    /// it was receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn insert_pending_request(
        &mut self,
        id: RequestId,
        pending: PendingRequest,
    ) -> (RequestReceipt, Option<InboundResponseHashes>) {
        let receipt = pending.receipt();
        let displaced = self.pending_requests.insert(id, pending).and_then(|displaced| {
            log::debug!("link({}): request {} displaced a pending entry with the same id", self.id, id);
            displaced.fail(RequestError::Send)
        });
        (receipt, displaced)
    }

    /// Forgets a request whose send failed; its receipt reads `LinkClosed`.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn remove_pending_request(&mut self, id: &RequestId) {
        self.pending_requests.remove(id);
        self.pending_response_limits.remove(id.as_bytes().as_slice());
    }

    /// A response for `id` arrived (`RNS/Link.py:857-884`): the receipt
    /// becomes `Ready` with the decoded body, or `Failed(Decode)`. An id
    /// with no pending request is a response nobody is waiting for. The
    /// response size limit goes with the entry.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn resolve_pending_request(&mut self, id: RequestId, arrival: ResponseArrival<'_>) {
        self.pending_response_limits.remove(id.as_bytes().as_slice());
        let Some(pending) = self.pending_requests.remove(&id) else {
            log::trace!("link({}): response to {} has no pending request", self.id, id);
            return;
        };
        log::debug!("link({}): request {} answered", self.id, id);
        if let Some(cause) = pending.resolve(arrival) {
            log::debug!("link({}): response to {} could not be decoded: {cause}", self.id, id);
        }
    }

    /// Fails the request `id` and drops its response size limit; the limit
    /// goes even when no receipt is pending under `id`. Returns the
    /// response Resource it was receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn fail_pending_request(
        &mut self,
        id: &RequestId,
        error: RequestError,
    ) -> Option<InboundResponseHashes> {
        self.pending_response_limits.remove(id.as_bytes().as_slice());
        let pending = self.pending_requests.remove(id)?;
        log::debug!("link({}): request {} failed: {error}", self.id, id);
        pending.fail(error)
    }

    /// Fails every pending request and drops their response size limits;
    /// the link is closed or starting over. Returns the response Resources
    /// they were receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn fail_all_pending_requests(
        &mut self,
        error: RequestError,
    ) -> Vec<InboundResponseHashes> {
        let mut receiving = Vec::new();
        for (id, pending) in self.pending_requests.drain() {
            self.pending_response_limits.remove(id.as_bytes().as_slice());
            log::debug!("link({}): request {} failed: {error}", self.id, id);
            receiving.extend(pending.fail(error));
        }
        receiving
    }

    /// `fail_all_pending_requests(LinkClosed)` from a path that holds the
    /// link alone: the link is closing or starting over under its own
    /// guard, and no ResourceManager is in reach. A response Resource still
    /// inbound is not cancelled here (`RNS/Resource.py:1111`): the transport
    /// drops a closed link's receivers with its state on the next link
    /// check, and a receiver left on a restarted link fails on its own
    /// retry limit.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn abandon_pending_requests(&mut self) {
        let receiving = self.fail_all_pending_requests(RequestError::LinkClosed);
        if !receiving.is_empty() {
            log::debug!(
                "link({}): {} response resource(s) left inbound, dropped with the link state",
                self.id,
                receiving.len()
            );
        }
    }

    /// A segment of the response to `id` was advertised and admitted: the
    /// ResourceManager built a receiver for it (`RNS/Link.py:1055-1064`,
    /// `:1420`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn mark_pending_request_receiving(
        &mut self,
        id: &RequestId,
        advertisement: &ResourceAdvertisement,
    ) {
        if let Some(pending) = self.pending_requests.get_mut(id) {
            pending.mark_receiving(advertisement);
        }
    }

    /// Progress on the response Resource segment `hash` (`RNS/Link.py:1429`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn note_pending_response_progress(&self, hash: Hash, progress: &ResourceProgress) {
        if let Some(pending) =
            self.pending_requests.values().find(|pending| pending.response_segment_hash() == Some(hash))
        {
            pending.note_progress(progress);
        }
    }

    /// A Resource of this link reached an outcome that concerns a pending
    /// request. For the request Resource `hash` (`RNS/Link.py:1367-1385`):
    /// complete is `Delivered`; a rejection, a failure or a local cancel
    /// fails the receipt.
    /// For the response Resource `hash`, an inbound failure fails the
    /// receipt `Receive`; nothing else expires a receipt that is
    /// `Receiving`. Other event kinds are not outcomes. Returns the response
    /// Resource a failed receipt was receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn note_request_resource_outcome(
        &mut self,
        hash: Hash,
        kind: &ResourceEventKind,
    ) -> Option<InboundResponseHashes> {
        if let ResourceEventKind::InboundFailed(_) = kind {
            let id = self
                .pending_requests
                .iter()
                .find(|(_, pending)| pending.response_original_hash() == Some(hash))
                .map(|(id, _)| *id)?;
            return self.fail_pending_request(&id, RequestError::Receive);
        }
        let outcome = match kind {
            ResourceEventKind::OutboundComplete => Ok(()),
            ResourceEventKind::OutboundRejected => Err(RequestError::Rejected),
            ResourceEventKind::OutboundFailed | ResourceEventKind::OutboundCancelled => {
                Err(RequestError::Send)
            }
            _ => return None,
        };
        let id = self
            .pending_requests
            .iter()
            .find(|(_, pending)| pending.request_resource_hash() == Some(hash))
            .map(|(id, _)| *id)?;
        match outcome {
            Ok(()) => {
                log::debug!("link({}): request {} delivered as a resource", self.id, id);
                if let Some(pending) = self.pending_requests.get(&id) {
                    pending.mark_delivered();
                }
                None
            }
            Err(error) => self.fail_pending_request(&id, error),
        }
    }

    /// Fails every request whose clock has run out with `Failed(Timeout)`.
    /// The clock runs from the send for a request packet and from
    /// `Delivered` for a request Resource (`RNS/Link.py:1331-1334`,
    /// `:1370-1373`), and not at all while a response Resource is being
    /// received (`:1389`): that transfer's own failure fails the receipt.
    /// Python (`:1398`, `:1408`) also leaves a receipt that never reached
    /// `DELIVERED` pending forever; here an unanswered request packet times
    /// out as well.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn expire_pending_requests(&mut self, now: Instant) {
        let expired: Vec<RequestId> = self
            .pending_requests
            .iter()
            .filter(|(_, pending)| pending.deadline().is_some_and(|deadline| deadline <= now))
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            // `deadline` is `None` while `Receiving`, so nothing is inbound.
            let receiving = self.fail_pending_request(&id, RequestError::Timeout);
            debug_assert!(receiving.is_none(), "an expired receipt has no response Resource inbound");
        }
    }

    /// The first request timeout due, over the entries whose clock is
    /// running; see `expire_pending_requests`.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn earliest_pending_request_deadline(&self) -> Option<Instant> {
        self.pending_requests.values().filter_map(PendingRequest::deadline).min()
    }
}
