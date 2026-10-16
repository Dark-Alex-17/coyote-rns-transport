// The requester's pending-request table, `Link.pending_requests` in
// `RNS/Link.py`. Every method is synchronous and runs under the link lock;
// the packet and resource paths that receive a response call in from there,
// and the watchdog expires timeouts from `check_watchdog`.

impl Link {
    /// Registers a request just sent and returns the requester's handle.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn insert_pending_request(
        &mut self,
        id: RequestId,
        pending: PendingRequest,
    ) -> RequestReceipt {
        let receipt = pending.receipt();
        if self.pending_requests.insert(id, pending).is_some() {
            log::debug!("link({}): request {} replaced a pending entry with the same id", self.id, id);
        }
        receipt
    }

    /// Forgets a request whose send failed; its receipt reads `LinkClosed`.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn remove_pending_request(&mut self, id: &RequestId) {
        self.pending_requests.remove(id);
    }

    /// A response for `id` arrived (`RNS/Link.py:857-884`): the receipt
    /// becomes `Ready` with the decoded body, or `Failed(Decode)`. An id
    /// with no pending request is a response nobody is waiting for.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn resolve_pending_request(&mut self, id: RequestId, arrival: ResponseArrival<'_>) {
        let Some(pending) = self.pending_requests.remove(&id) else {
            log::trace!("link({}): response to {} has no pending request", self.id, id);
            return;
        };
        log::debug!("link({}): request {} answered", self.id, id);
        pending.resolve(arrival);
    }

    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn fail_pending_request(&mut self, id: &RequestId, error: RequestError) {
        if let Some(pending) = self.pending_requests.remove(id) {
            log::debug!("link({}): request {} failed: {error}", self.id, id);
            pending.fail(error);
        }
    }

    /// Fails every pending request; the link is closed or starting over.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn fail_all_pending_requests(&mut self, error: RequestError) {
        for (id, pending) in self.pending_requests.drain() {
            log::debug!("link({}): request {} failed: {error}", self.id, id);
            pending.fail(error);
        }
    }

    /// The response to `id` was advertised as a Resource of `response_size`
    /// data bytes and `response_transfer_size` bytes on the wire
    /// (`RNS/Link.py:870-873`, `:1420`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn mark_pending_request_receiving(
        &mut self,
        id: &RequestId,
        response_size: u64,
        response_transfer_size: u64,
        response_hash: Hash,
    ) {
        if let Some(pending) = self.pending_requests.get_mut(id) {
            pending.mark_receiving(response_size, response_transfer_size, response_hash);
        }
    }

    /// Progress on the response Resource `hash` (`RNS/Link.py:1430`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn note_pending_response_progress(&self, hash: Hash, progress: &ResourceProgress) {
        if let Some(pending) =
            self.pending_requests.values().find(|pending| pending.response_resource_hash() == Some(hash))
        {
            pending.note_progress(progress);
        }
    }

    /// The request Resource `hash` reached a sender-side outcome
    /// (`RNS/Link.py:1367-1385`): complete is `Delivered`, a rejection or a
    /// failure fails the receipt. Other event kinds are not outcomes.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn note_request_resource_outcome(&mut self, hash: Hash, kind: &ResourceEventKind) {
        let outcome = match kind {
            ResourceEventKind::OutboundComplete => Ok(()),
            ResourceEventKind::OutboundRejected => Err(RequestError::Rejected),
            ResourceEventKind::OutboundFailed => Err(RequestError::Send),
            _ => return,
        };
        let Some(id) = self
            .pending_requests
            .iter()
            .find(|(_, pending)| pending.request_resource_hash() == Some(hash))
            .map(|(id, _)| *id)
        else {
            return;
        };
        match outcome {
            Ok(()) => {
                log::debug!("link({}): request {} delivered as a resource", self.id, id);
                if let Some(pending) = self.pending_requests.get(&id) {
                    pending.mark_delivered();
                }
            }
            Err(error) => self.fail_pending_request(&id, error),
        }
    }

    /// Fails every request whose deadline has passed with
    /// `Failed(Timeout)`, whatever its status. Python fails only a receipt
    /// that reached `DELIVERED` (`RNS/Link.py:1398`, `:1408`), which leaves
    /// a packet-form request that was never answered pending forever.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn expire_pending_requests(&mut self, now: Instant) {
        let expired: Vec<RequestId> = self
            .pending_requests
            .iter()
            .filter(|(_, pending)| pending.deadline() <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            self.fail_pending_request(&id, RequestError::Timeout);
        }
    }

    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn earliest_pending_request_deadline(&self) -> Option<Instant> {
        self.pending_requests.values().map(PendingRequest::deadline).min()
    }
}
