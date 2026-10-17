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

    /// Drops the inbound transfer whose in-flight segment is `segment_hash`
    /// and reports it `InboundFailed("cancelled")` under its original hash,
    /// as a remote cancel would. Returns that original hash, `None` when no
    /// such transfer is in progress. Sending the receiver cancel is the
    /// transport's job (`RNS/Resource.py:1111-1116`).
    pub(crate) fn cancel_incoming(&mut self, segment_hash: Hash) -> Option<Hash> {
        let receiver = self.incoming.remove(&segment_hash)?;
        let original_hash = receiver.original_hash;
        if !self.fail_inbound_segments(original_hash, "cancelled") {
            self.events.push(ResourceEvent {
                hash: original_hash,
                link_id: receiver.link_id,
                kind: ResourceEventKind::InboundFailed(ResourceFailure {
                    reason: "cancelled".to_string(),
                    progress: receiver.progress(),
                }),
            });
        }
        Some(original_hash)
    }

    /// `cancel_incoming` for a response whose receipt failed, in whichever
    /// form the transfer is: a live receiver under `segment_hash`, or, between
    /// one segment's completion and the next segment's admitted
    /// advertisement, only the partial assembly under `original_hash`, which
    /// would otherwise sit there until the link closes. Either way the
    /// transfer is reported `InboundFailed("cancelled")`. A transfer under
    /// those hashes on a link other than `link_id` is left alone: the
    /// hashes came from that link's receipt. Returns whether a receiver was
    /// in flight, which is when the peer is owed a receiver cancel.
    pub(crate) fn cancel_incoming_response(
        &mut self,
        segment_hash: Hash,
        original_hash: Hash,
        link_id: &AddressHash,
    ) -> bool {
        if self.incoming.get(&segment_hash).is_some_and(|receiver| receiver.link_id == *link_id)
            && self.cancel_incoming(segment_hash).is_some()
        {
            return true;
        }
        if self
            .incoming_segments
            .get(&original_hash)
            .is_some_and(|assembly| assembly.link_id == *link_id)
        {
            self.fail_inbound_segments(original_hash, "cancelled");
        }
        false
    }
}
