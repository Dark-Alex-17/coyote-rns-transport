/// What `cancel_incoming_response` did: whether a receiver was in flight,
/// which is when the peer is owed a receiver cancel, and the transfer's
/// `InboundFailed("cancelled")`, handed back for the caller to publish
/// rather than queued. It never sits in the shared event queue because the
/// next routed drain would key it by hash into the pending requests, where
/// a fresh record may be receiving a retry of the very same transfer.
#[must_use = "the event is for the caller to publish"]
pub(crate) struct CancelledResponse {
    pub(crate) receiver_in_flight: bool,
    pub(crate) event: Option<ResourceEvent>,
}

impl ResourceManager {
    pub fn remove_link_state(&mut self, link_id: AddressHash) {
        let mut failed_outbound = std::collections::HashSet::new();
        let outgoing = std::mem::take(&mut self.outgoing);
        for (hash, sender) in outgoing {
            if sender.link_id == link_id {
                if failed_outbound.insert(sender.original_hash) {
                    self.events.push(ResourceEvent {
                        hash: sender.original_hash,
                        link_id,
                        kind: ResourceEventKind::OutboundFailed,
                    });
                }
            } else {
                self.outgoing.insert(hash, sender);
            }
        }
        // A split transfer has one chain keyed by its original hash alongside
        // the currently active segment. Keep the terminal signal singular even
        // when link cleanup observes both structures, and still report a chain
        // whose active segment has already been removed by an earlier failure.
        let outgoing_segment_chains = std::mem::take(&mut self.outgoing_segment_chains);
        for (hash, pending) in outgoing_segment_chains {
            if pending.link_id == link_id {
                if failed_outbound.insert(pending.original_hash) {
                    self.events.push(ResourceEvent {
                        hash: pending.original_hash,
                        link_id,
                        kind: ResourceEventKind::OutboundFailed,
                    });
                }
            } else {
                self.outgoing_segment_chains.insert(hash, pending);
            }
        }

        let mut failed_inbound_segments = std::collections::HashSet::new();
        let incoming_segments = std::mem::take(&mut self.incoming_segments);
        for (hash, assembly) in incoming_segments {
            if assembly.link_id == link_id {
                failed_inbound_segments.insert(hash);
                self.events.push(ResourceEvent {
                    hash,
                    link_id,
                    kind: ResourceEventKind::InboundFailed(ResourceFailure {
                        reason: "link_closed".to_string(),
                        progress: ResourceProgress {
                            received_bytes: assembly.data.len() as u64,
                            total_bytes: assembly.total_data_size,
                            received_parts: assembly.next_segment.saturating_sub(1) as usize,
                            total_parts: assembly.total_segments as usize,
                        },
                    }),
                });
            } else {
                self.incoming_segments.insert(hash, assembly);
            }
        }
        let incoming = std::mem::take(&mut self.incoming);
        for (hash, receiver) in incoming {
            if receiver.link_id == link_id {
                // Segment n has a live receiver while the completed segments
                // remain in `incoming_segments` under the same original hash.
                // The assembly event above is the logical transfer failure;
                // avoid emitting a second terminal callback for that transfer.
                if !failed_inbound_segments.contains(&receiver.original_hash) {
                    self.events.push(ResourceEvent {
                        hash: receiver.original_hash,
                        link_id,
                        kind: ResourceEventKind::InboundFailed(ResourceFailure {
                            reason: "link_closed".to_string(),
                            progress: receiver.progress(),
                        }),
                    });
                }
            } else {
                self.incoming.insert(hash, receiver);
            }
        }
        self.link_stats.remove(&link_id);
    }

    /// Whether an inbound transfer is being received under the segment hash
    /// `segment_hash`: an advertisement the manager admitted and built a
    /// receiver for, as opposed to one it dropped, and whose receiver was
    /// built for `request_id` on `link_id`, so a re-advertisement of an
    /// active segment under another request id marks nothing.
    pub(crate) fn has_active_receiver(
        &self,
        segment_hash: &Hash,
        link_id: &AddressHash,
        request_id: &[u8],
    ) -> bool {
        self.incoming.get(segment_hash).is_some_and(|receiver| {
            receiver.is_active()
                && receiver.link_id == *link_id
                && receiver.request_id.as_deref() == Some(request_id)
        })
    }

    /// Drops the inbound transfer whose in-flight segment is `segment_hash`,
    /// as a remote cancel would, and builds its `InboundFailed("cancelled")`
    /// under the original hash: when the earlier segments of a split
    /// transfer are assembled under that hash, the assembly goes too and its
    /// event is the one, else the receiver's. `None` when no receiver is in
    /// flight. Sending the receiver cancel is the transport's job
    /// (`RNS/Resource.py:1111-1116`).
    fn take_cancelled_incoming(&mut self, segment_hash: Hash) -> Option<ResourceEvent> {
        let receiver = self.incoming.remove(&segment_hash)?;
        let original_hash = receiver.original_hash;
        Some(self.take_inbound_segments(original_hash, "cancelled").unwrap_or_else(|| {
            ResourceEvent {
                hash: original_hash,
                link_id: receiver.link_id,
                kind: ResourceEventKind::InboundFailed(ResourceFailure {
                    reason: "cancelled".to_string(),
                    progress: receiver.progress(),
                }),
            }
        }))
    }

    /// Cancels a response whose receipt failed, in whichever form the
    /// transfer is: a live receiver under `segment_hash`, or, between one
    /// segment's completion and the next segment's admitted advertisement,
    /// only the partial assembly under `original_hash`, which would
    /// otherwise sit there until the link closes. Either way the transfer's
    /// `InboundFailed("cancelled")` is handed back, not queued. A transfer
    /// under those hashes on a link other than `link_id` is left alone: the
    /// hashes came from that link's receipt.
    pub(crate) fn cancel_incoming_response(
        &mut self,
        segment_hash: Hash,
        original_hash: Hash,
        link_id: &AddressHash,
    ) -> CancelledResponse {
        if self.incoming.get(&segment_hash).is_some_and(|receiver| receiver.link_id == *link_id) {
            if let Some(event) = self.take_cancelled_incoming(segment_hash) {
                return CancelledResponse { receiver_in_flight: true, event: Some(event) };
            }
        }
        let event = self
            .incoming_segments
            .get(&original_hash)
            .is_some_and(|assembly| assembly.link_id == *link_id)
            .then(|| self.take_inbound_segments(original_hash, "cancelled"))
            .flatten();
        CancelledResponse { receiver_in_flight: false, event }
    }
}
