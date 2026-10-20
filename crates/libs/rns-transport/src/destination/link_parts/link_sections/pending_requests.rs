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
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn insert_pending_request(
        &mut self,
        id: RequestId,
        pending: PendingRequest,
    ) -> (RequestReceipt, Option<InboundResponseHashes>) {
        let receipt = pending.receipt();
        let displaced = self.pending_requests.insert(id, pending).and_then(|displaced| {
            log::debug!(
                "link({}): request {} displaced a pending entry with the same id",
                self.id,
                id
            );
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
    /// response size limit goes with the entry, after an assembled response
    /// Resource is measured against it once more and fails the receipt
    /// `Oversize` when it is over: the advertised `d` the gate read was the
    /// peer's claim, and an uncompressed response carries up to `t` bytes.
    /// Python trusts the advertisement (`RNS/Link.py:1049`). Returns the
    /// response Resource an oversize receipt was receiving, for the caller
    /// to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn resolve_pending_request(
        &mut self,
        id: RequestId,
        arrival: ResponseArrival<'_>,
    ) -> Option<InboundResponseHashes> {
        if let ResponseArrival::Resource { data, .. } = &arrival {
            if let Some(limit) = self.response_size_limit(id.as_bytes()) {
                if data.len() > limit {
                    return self.fail_pending_request(&id, RequestError::Oversize);
                }
            }
        }
        self.pending_response_limits.remove(id.as_bytes().as_slice());
        let Some(pending) = self.pending_requests.remove(&id) else {
            log::trace!("link({}): response to {} has no pending request", self.id, id);
            return None;
        };
        log::debug!("link({}): request {} answered", self.id, id);
        if let Some(cause) = pending.resolve(arrival) {
            log::debug!("link({}): response to {} could not be decoded: {cause}", self.id, id);
        }
        None
    }

    /// Fails the request `id` and drops its response size limit; the limit
    /// goes even when no receipt is pending under `id`. Returns the response
    /// Resource the receipt, or the record no receipt owns, was receiving,
    /// for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn fail_pending_request(
        &mut self,
        id: &RequestId,
        error: RequestError,
    ) -> Option<InboundResponseHashes> {
        let removed = self.pending_response_limits.remove(id.as_bytes().as_slice());
        let record_receiving = removed.and_then(PendingResponseLimit::into_receiving);
        let Some(pending) = self.pending_requests.remove(id) else {
            return record_receiving;
        };
        log::debug!("link({}): request {} failed: {error}", self.id, id);
        let pending_receiving = pending.fail(error);
        debug_assert!(
            pending_receiving.is_none() || record_receiving.is_none(),
            "a receipt and its record never both hold the inbound response"
        );
        pending_receiving.or(record_receiving)
    }

    /// The response Resources owed a cancel: a receipt or a record failed or
    /// displaced under the link lock while receiving (an over-limit packet
    /// response, a request re-sent under a receiving id, `expect_response`
    /// over a receiving record) cannot reach the ResourceManager, so the
    /// transport drains this on the link's next inbound packet, Resource
    /// packets included, and cancels each one (`RNS/Link.py:1435`).
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn take_owed_response_cancels(&mut self) -> Vec<InboundResponseHashes> {
        std::mem::take(&mut self.owed_response_cancels)
    }

    /// Fails every pending request and drops their response size limits;
    /// the link is closed or starting over, or the transport is stopping.
    /// Returns the response Resources they were receiving, for the caller
    /// to cancel.
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
    /// retry limit. The cancels the link still owed go the same way, with
    /// the link state, and so does the whole limits table: a record no
    /// receipt owns belongs to the old link as much as a receipt does.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn abandon_pending_requests(&mut self) {
        let receiving = self.fail_all_pending_requests(RequestError::LinkClosed);
        self.pending_response_limits.clear();
        self.owed_response_cancels.clear();
        if !receiving.is_empty() {
            log::debug!(
                "link({}): {} response resource(s) left inbound, dropped with the link state",
                self.id,
                receiving.len()
            );
        }
    }

    /// A segment of the response to `request_id` was advertised and
    /// admitted: the ResourceManager built a receiver for it
    /// (`RNS/Link.py:1055-1064`, `:1420`). A receipt under the id takes it;
    /// an entry no receipt owns stops its clock until that Resource's own
    /// outcome clears it (`:1389`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn mark_pending_request_receiving(
        &mut self,
        request_id: &[u8],
        advertisement: &ResourceAdvertisement,
    ) {
        let receipt_id = <[u8; ADDRESS_HASH_SIZE]>::try_from(request_id).ok().map(RequestId::new);
        if let Some(pending) = receipt_id.and_then(|id| self.pending_requests.get_mut(&id)) {
            pending.mark_receiving(advertisement);
        } else if let Some(entry) = self.pending_response_limits.get_mut(request_id) {
            // The next segment of the same response replaces the previous
            // one's hashes; nothing is owed.
            entry.clock = RecordClock::Receiving(InboundResponseHashes {
                segment: advertisement.hash,
                original: advertisement.original_hash,
            });
        }
    }

    /// Progress on the response Resource segment `hash` (`RNS/Link.py:1429`).
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn note_pending_response_progress(&self, hash: Hash, progress: &ResourceProgress) {
        if let Some(pending) = self
            .pending_requests
            .values()
            .find(|pending| pending.response_segment_hash() == Some(hash))
        {
            pending.note_progress(progress);
        }
    }

    /// A Resource of this link reached an outcome that concerns a pending
    /// request. For the request Resource `hash` (`RNS/Link.py:1367-1385`):
    /// complete is `Delivered`; a rejection, a failure or a local cancel
    /// fails the receipt, and an entry no receipt owns is armed or dropped
    /// the same way.
    /// For the response Resource `hash`, an inbound failure fails the
    /// receipt `Receive` and clears the entry receiving it; nothing else
    /// expires either while `Receiving`. Other event kinds are not
    /// outcomes. Returns the response Resource a failed receipt or a
    /// dropped entry was receiving, for the caller to cancel.
    #[deny(clippy::await_holding_invalid_type)]
    #[must_use = "the hashes are for the caller to cancel"]
    pub(crate) fn note_request_resource_outcome(
        &mut self,
        hash: Hash,
        kind: &ResourceEventKind,
    ) -> Option<InboundResponseHashes> {
        if let ResourceEventKind::InboundFailed(_) = kind {
            self.pending_response_limits.retain(|_, entry| {
                !matches!(entry.clock, RecordClock::Receiving(inbound) if inbound.original == hash)
            });
            let id = self
                .pending_requests
                .iter()
                .find(|(_, pending)| pending.response_original_hash() == Some(hash))
                .map(|(id, _)| *id)?;
            return self.fail_pending_request(&id, RequestError::Receive);
        }
        let outcome = match kind {
            ResourceEventKind::OutboundComplete => Ok(()),
            ResourceEventKind::OutboundRejected { .. } => Err(RequestError::Rejected),
            ResourceEventKind::OutboundFailed | ResourceEventKind::OutboundCancelled => {
                Err(RequestError::Send)
            }
            _ => return None,
        };
        let mut dropped = None;
        if outcome.is_ok() {
            let clock = self.pending_response_expiry();
            for entry in self.pending_response_limits.values_mut() {
                if entry.request_resource == Some(hash) && entry.clock == RecordClock::Unarmed {
                    entry.clock = clock;
                }
            }
        } else {
            self.pending_response_limits.retain(|_, entry| {
                if entry.request_resource != Some(hash) {
                    return true;
                }
                if let RecordClock::Receiving(inbound) = entry.clock {
                    dropped = Some(inbound);
                }
                false
            });
        }
        debug_assert!(
            dropped.is_none()
                || !self
                    .pending_requests
                    .values()
                    .any(|pending| pending.request_resource_hash() == Some(hash)),
            "a request Resource is owned by a receipt or a record, never both"
        );
        let Some(id) = self
            .pending_requests
            .iter()
            .find(|(_, pending)| pending.request_resource_hash() == Some(hash))
            .map(|(id, _)| *id)
        else {
            return dropped;
        };
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
    /// out as well. A response entry no receipt owns follows the same
    /// clock and goes once it runs out; one owned by a live receipt is left
    /// to the receipt. Python's marker is the `RequestReceipt`: one
    /// `DELIVERED` past its timeout leaves `pending_requests`
    /// (`:1397-1405`), one `RECEIVING` is never timed out (`:1389`), and
    /// one that never reached `DELIVERED` stays, so a packet-form marker
    /// there can outlive the entry pruned here.
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
            debug_assert!(
                receiving.is_none(),
                "an expired receipt has no response Resource inbound"
            );
        }
        let pending = &self.pending_requests;
        let before = self.pending_response_limits.len();
        self.pending_response_limits
            .retain(|id, entry| owned_by_receipt(pending, id) || !entry.clock.has_run_out(now));
        let pruned = before - self.pending_response_limits.len();
        if pruned > 0 {
            log::debug!("link({}): {pruned} unanswered request entries expired", self.id);
        }
    }

    /// The first request timeout due, over the entries whose clock is
    /// running, and the expiry of every response entry no receipt owns;
    /// see `expire_pending_requests`.
    #[deny(clippy::await_holding_invalid_type)]
    pub(crate) fn earliest_pending_request_deadline(&self) -> Option<Instant> {
        let receipts = self.pending_requests.values().filter_map(PendingRequest::deadline);
        let unowned =
            self.pending_response_limits.iter().filter_map(|(id, entry)| match entry.clock {
                RecordClock::Expires(at) if !owned_by_receipt(&self.pending_requests, id) => {
                    Some(at)
                }
                _ => None,
            });
        receipts.chain(unowned).min()
    }
}

/// Whether a live receipt under `id` owns the response entry recorded for it.
fn owned_by_receipt(pending: &HashMap<RequestId, PendingRequest>, id: &[u8]) -> bool {
    <[u8; ADDRESS_HASH_SIZE]>::try_from(id)
        .ok()
        .map(RequestId::new)
        .is_some_and(|id| pending.contains_key(&id))
}
