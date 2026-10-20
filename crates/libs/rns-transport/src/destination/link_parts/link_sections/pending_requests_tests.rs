// Kept separate from `pending_requests.rs` so that file stays below the
// repository's 500-line module limit.
#[cfg(test)]
mod pending_requests_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    use crate::destination::request::{RequestStatus, ResponseFrame};
    use crate::resource::{RejectReason, ResourceFailure, ResourceProgress, RANDOM_HASH_SIZE};

    const TIMEOUT: Duration = Duration::from_secs(5);

    pub(super) fn id(byte: u8) -> RequestId {
        RequestId::new([byte; ADDRESS_HASH_SIZE])
    }

    pub(super) fn hash(byte: u8) -> Hash {
        Hash::new_from_slice(&[byte; HASH_SIZE])
    }

    pub(super) fn hashes(segment: u8, original: u8) -> InboundResponseHashes {
        InboundResponseHashes { segment: hash(segment), original: hash(original) }
    }

    pub(super) fn response_segment(segment_hash: Hash, original: Hash) -> ResourceAdvertisement {
        ResourceAdvertisement {
            transfer_size: 10,
            data_size: 10,
            parts: 1,
            hash: segment_hash,
            random_hash: [0; RANDOM_HASH_SIZE],
            original_hash: original,
            segment_index: 1,
            total_segments: 1,
            request_id: None,
            flags: 0,
            hashmap: Vec::new(),
        }
    }

    fn inbound_failed() -> ResourceEventKind {
        ResourceEventKind::InboundFailed(ResourceFailure {
            reason: "retry_limit_exhausted".to_string(),
            progress: ResourceProgress {
                received_bytes: 0,
                total_bytes: 10,
                received_parts: 0,
                total_parts: 1,
            },
        })
    }

    /// A packet-form request with a response size limit registered.
    fn pending_packet_request(link: &mut Link, id: RequestId, timeout: Duration) -> RequestReceipt {
        link.set_response_size_limit(id.as_bytes(), 1024);
        link.insert_pending_request(id, PendingRequest::new(id, timeout, None)).0
    }

    /// A resource-form request whose request Resource is `request_hash`.
    fn pending_resource_request(
        link: &mut Link,
        id: RequestId,
        request_hash: Hash,
    ) -> RequestReceipt {
        link.insert_pending_request(id, PendingRequest::new(id, TIMEOUT, Some(request_hash))).0
    }

    /// `RNS/Link.py:1046-1049` reads the gate at every segment advertisement: an
    /// advertisement the ResourceManager drops leaves the limit where it was, the next one
    /// is measured against it, a receipt without a limit admits any size, an id nobody
    /// waits for solicits nothing, and a `send_request_resource*` id of any length is
    /// solicited by its entry alone and measured once more when its response is assembled.
    /// The limit goes when the response resolves.
    #[test]
    fn the_response_size_gate_is_read_per_advertisement_and_not_spent_by_one() {
        let (mut link, _, _, _) = linked_pair();
        let bounded = pending_packet_request(&mut link, id(1), TIMEOUT);
        let bounded_id = id(1);
        let q = bounded_id.as_bytes().as_slice();
        assert_eq!(link.response_size_allowed(q, 1024), Some(true), "d == limit is admitted");
        assert_eq!(link.response_size_allowed(q, 1024), Some(true), "and not spent");
        assert_eq!(link.response_size_allowed(q, 1025), Some(false), "d == limit + 1 is refused");
        assert_eq!(link.response_size_limit(q), Some(1024), "refusing leaves the limit in place");

        let _unbounded = pending_resource_request(&mut link, id(2), hash(0xA2));
        assert_eq!(link.response_size_allowed(id(2).as_bytes(), usize::MAX), Some(true));
        assert_eq!(link.response_size_allowed(id(3).as_bytes(), 1), None, "nobody asked");

        link.expect_response(b"legacy-but-longer", None);
        assert_eq!(link.response_size_allowed(b"legacy-but-longer", usize::MAX), Some(true));
        link.expect_response(b"legacy-but-longer", Some(10));
        assert_eq!(link.response_size_allowed(b"legacy-but-longer", 11), Some(false));
        assert!(!link.take_response_limit_if_allowed(b"legacy-but-longer", 11), "assembled over");
        assert_eq!(link.response_size_allowed(b"legacy-but-longer", 1), None, "and went with it");
        link.note_request_sent(q);
        assert_eq!(link.response_size_limit(q), Some(1024), "the send keeps a limit set before it");
        link.note_request_sent(b"fresh");
        assert_eq!(link.response_size_allowed(b"fresh", usize::MAX), Some(true), "sent, no limit");

        let envelope = ResponseFrame::new(id(1), rmpv::Value::from(7)).encode();
        let receiving = link.resolve_pending_request(
            id(1),
            ResponseArrival::Resource { data: &envelope, metadata: None },
        );
        assert!(receiving.is_none(), "a response within its limit resolves");
        assert_eq!(bounded.status(), RequestStatus::Ready);
        assert_eq!(link.response_size_allowed(q, 1), None, "the limit went with the entry");
    }

    /// The advertised `d` the gate read is the peer's claim; an uncompressed response
    /// Resource carries up to `t` bytes, so the assembled data is measured against the
    /// limit once more and fails the receipt `Oversize` when over it, handing back the
    /// segment it was receiving. Python trusts the advertisement (`RNS/Link.py:1049`).
    /// The bound is inclusive, as at the advertisement.
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn an_assembled_response_over_its_limit_fails_the_receipt_oversize() {
        let (mut link, _, _, _) = linked_pair();
        let lied_to = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(1), hash(1)));
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 10), Some(true), "d claims 10");

        let envelope = ResponseFrame::new(id(1), rmpv::Value::Binary(vec![0u8; 2000])).encode();
        let receiving = link.resolve_pending_request(
            id(1),
            ResponseArrival::Resource { data: &envelope, metadata: None },
        );
        assert_eq!(receiving, Some(hashes(1, 1)), "the segment in flight and the whole");
        assert_eq!(lied_to.status(), RequestStatus::Failed);
        assert_eq!(lied_to.response().await, Err(RequestError::Oversize));
        assert!(link.pending_requests.is_empty());
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 1), None, "the limit went with it");

        let at_the_bound = pending_packet_request(&mut link, id(2), TIMEOUT);
        let probe = ResponseFrame::new(id(2), rmpv::Value::Binary(vec![0u8; 1000])).encode();
        let overhead = probe.len() - 1000;
        let envelope =
            ResponseFrame::new(id(2), rmpv::Value::Binary(vec![0u8; 1024 - overhead])).encode();
        assert_eq!(envelope.len(), 1024, "the envelope is exactly the limit");
        let receiving = link.resolve_pending_request(
            id(2),
            ResponseArrival::Resource { data: &envelope, metadata: None },
        );
        assert!(receiving.is_none());
        assert_eq!(at_the_bound.status(), RequestStatus::Ready, "the bound is inclusive");
        assert_eq!(link.response_size_allowed(id(2).as_bytes(), 1), None);
    }

    #[test]
    fn a_receiving_entry_is_not_expired_and_not_folded_into_the_watchdog() {
        let (mut link, _, _, _) = linked_pair();
        let receiving = pending_packet_request(&mut link, id(1), Duration::ZERO);
        let waiting = pending_packet_request(&mut link, id(2), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(1), hash(1)));
        assert_eq!(receiving.status(), RequestStatus::Receiving);

        let far = Instant::now() + TIMEOUT / 2;
        link.expire_pending_requests(far);
        assert_eq!(receiving.status(), RequestStatus::Receiving, "no clock runs while receiving");
        assert_eq!(waiting.status(), RequestStatus::Sent);
        let deadline = link.earliest_pending_request_deadline().expect("the waiting entry");
        assert!(deadline > far, "the receiving entry's expired deadline is not folded in");
        let keepalive = link.keepalive_deadline(true).expect("an active link has a keepalive");
        assert_eq!(link.next_watchdog_deadline(true), Some(deadline.min(keepalive)));
    }

    /// A response entry no receipt owns, the send-time record of a bare request packet
    /// or a `send_request_resource*` entry with nothing inbound, is pruned by the watchdog
    /// at the default request timeout. Python's marker is the `RequestReceipt`: one
    /// `DELIVERED` past its timeout leaves `pending_requests` (`RNS/Link.py:1397-1405`),
    /// one that never reached `DELIVERED` stays, so a packet-form marker there can outlive
    /// this entry. An entry owned by a live receipt, `Sent` or `Receiving`, is the
    /// receipt's to drop.
    #[test]
    fn an_entry_without_a_receipt_expires_on_the_default_request_timeout() {
        let (mut link, _, _, _) = linked_pair();
        let marker_timeout = RequestOptions::default().timeout_for_rtt(link.rtt());
        link.note_request_sent(b"fresh");
        link.expect_response(b"legacy", Some(10));
        let waiting = pending_packet_request(&mut link, id(1), Duration::from_secs(60));
        let receiving = pending_packet_request(&mut link, id(2), Duration::from_secs(60));
        link.mark_pending_request_receiving(id(2).as_bytes(), &response_segment(hash(2), hash(2)));
        assert_eq!(receiving.status(), RequestStatus::Receiving);

        let folded = link.earliest_pending_request_deadline().expect("the unowned entries");
        assert!(folded <= Instant::now() + marker_timeout, "the unowned entries wake the watchdog");
        assert!(folded < waiting.started_at().expect("sent") + Duration::from_secs(60));

        link.expire_pending_requests(Instant::now() + marker_timeout / 2);
        assert_eq!(link.response_size_allowed(b"fresh", 1), Some(true));
        assert_eq!(link.response_size_allowed(b"legacy", 1), Some(true));
        assert_eq!(link.response_size_limit(id(1).as_bytes()), Some(1024));
        assert_eq!(link.response_size_limit(id(2).as_bytes()), Some(1024));

        link.expire_pending_requests(Instant::now() + marker_timeout + Duration::from_millis(1));
        assert_eq!(link.response_size_allowed(b"fresh", 1), None, "the send-time record went");
        assert_eq!(link.response_size_allowed(b"legacy", 1), None, "the resource-form entry went");
        assert_eq!(link.response_size_limit(id(1).as_bytes()), Some(1024), "owned by a receipt");
        assert_eq!(waiting.status(), RequestStatus::Sent);
        assert_eq!(link.response_size_limit(id(2).as_bytes()), Some(1024), "owned by a receipt");
        assert_eq!(receiving.status(), RequestStatus::Receiving);
        assert_eq!(
            link.earliest_pending_request_deadline(),
            waiting.started_at().map(|sent| sent + Duration::from_secs(60)),
            "only the waiting receipt's clock is left"
        );
    }

    /// An entry no receipt owns stops its clock while a response Resource is inbound, as
    /// `RNS/Link.py:1389` never times out `RECEIVING`; that Resource's own outcome clears it.
    #[test]
    fn an_entry_without_a_receipt_is_pinned_while_its_response_is_inbound() {
        let (mut link, _, _, _) = linked_pair();
        let marker_timeout = RequestOptions::default().timeout_for_rtt(link.rtt());
        link.note_request_sent(id(1).as_bytes());
        link.expect_response(b"legacy", Some(10));
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(1), hash(3)));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(2), hash(4)));
        assert_eq!(link.earliest_pending_request_deadline(), None, "no clock runs while receiving");

        link.expire_pending_requests(Instant::now() + marker_timeout + Duration::from_millis(1));
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 1), Some(true));
        assert_eq!(link.response_size_allowed(b"legacy", 1), Some(true));

        assert_eq!(link.note_request_resource_outcome(hash(3), &inbound_failed()), None);
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 1), None, "cleared by its failure");
        assert_eq!(link.response_size_allowed(b"legacy", 1), Some(true), "the other is untouched");
        assert_eq!(link.note_request_resource_outcome(hash(4), &inbound_failed()), None);
        assert_eq!(link.response_size_allowed(b"legacy", 1), None);
    }

    /// A `send_request_resource*` entry waits behind its request Resource: no clock until
    /// `OutboundComplete` arms it (`RNS/Link.py:1370-1373`); a rejection or a failure of
    /// that Resource drops it.
    #[test]
    fn a_resource_form_entry_without_a_receipt_is_armed_by_its_request_resource() {
        let (mut link, _, _, _) = linked_pair();
        let marker_timeout = RequestOptions::default().timeout_for_rtt(link.rtt());
        assert_eq!(link.expect_response_to_resource(b"legacy", Some(10), hash(0xA1)), None);
        assert_eq!(link.expect_response_to_resource(b"refused", None, hash(0xA2)), None);
        assert_eq!(link.expect_response_to_resource(b"lost", None, hash(0xA3)), None);
        assert_eq!(link.earliest_pending_request_deadline(), None, "no clock before delivery");
        link.expire_pending_requests(Instant::now() + marker_timeout * 2);
        assert_eq!(link.response_size_allowed(b"legacy", 10), Some(true));
        assert_eq!(link.response_size_allowed(b"legacy", 11), Some(false));

        let complete = ResourceEventKind::OutboundComplete;
        assert_eq!(link.note_request_resource_outcome(hash(0xA1), &complete), None);
        let deadline = link.earliest_pending_request_deadline().expect("delivery arms the clock");
        assert!(deadline <= Instant::now() + marker_timeout);
        let rejected = ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled };
        assert_eq!(link.note_request_resource_outcome(hash(0xA2), &rejected), None);
        assert_eq!(link.response_size_allowed(b"refused", 1), None, "dropped with the request");
        let failed = ResourceEventKind::OutboundFailed;
        assert_eq!(link.note_request_resource_outcome(hash(0xA3), &failed), None);
        assert_eq!(link.response_size_allowed(b"lost", 1), None);
        link.expire_pending_requests(deadline + Duration::from_millis(1));
        assert_eq!(link.response_size_allowed(b"legacy", 1), None, "pruned at its deadline");
    }

    /// An entry whose clock ran out ahead of the watchdog admits nothing; the send of a
    /// request packet under its id restarts the clock and keeps the limit.
    #[test]
    fn a_run_out_clock_admits_nothing_and_the_send_restarts_it() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", Some(10));
        let ran_out = Instant::now() - Duration::from_secs(1);
        let entry = link.pending_response_limits.get_mut(b"legacy".as_slice()).expect("recorded");
        entry.clock = RecordClock::Expires(ran_out);
        assert_eq!(link.response_size_allowed(b"legacy", 1), None, "run out, not yet pruned");
        assert_eq!(link.response_size_limit(b"legacy"), Some(10), "the entry is still there");

        link.note_request_sent(b"legacy");
        assert_eq!(link.response_size_limit(b"legacy"), Some(10), "the send keeps the limit");
        assert!(link.earliest_pending_request_deadline() > Some(ran_out), "and restarts the clock");
        assert_eq!(link.response_size_allowed(b"legacy", 10), Some(true));
    }

    #[test]
    fn a_resource_form_entry_expires_from_delivered_not_from_send() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_resource_request(&mut link, id(1), hash(0xA1));
        assert_eq!(link.earliest_pending_request_deadline(), None);
        link.expire_pending_requests(Instant::now() + TIMEOUT * 2);
        assert_eq!(receipt.status(), RequestStatus::Sent, "no clock before delivery");

        assert_eq!(
            link.note_request_resource_outcome(hash(0xA1), &ResourceEventKind::OutboundComplete),
            None,
            "delivery fails nothing"
        );
        assert_eq!(receipt.status(), RequestStatus::Delivered);
        let delivered = receipt.started_at().expect("delivery starts the clock");
        assert_eq!(link.earliest_pending_request_deadline(), Some(delivered + TIMEOUT));
        link.expire_pending_requests(delivered + TIMEOUT);
        assert_eq!(receipt.status(), RequestStatus::Failed);
    }

    #[test]
    fn an_unbounded_timeout_never_expires_and_never_wakes_the_watchdog() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), Duration::MAX);
        assert_eq!(link.earliest_pending_request_deadline(), None);
        link.expire_pending_requests(Instant::now() + TIMEOUT);
        assert_eq!(receipt.status(), RequestStatus::Sent);
        assert_eq!(link.next_watchdog_deadline(true), link.keepalive_deadline(true));
    }

    #[test]
    fn a_failed_response_resource_fails_its_receipt_receive() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(2), hash(1)));

        assert_eq!(
            link.note_request_resource_outcome(hash(2), &inbound_failed()),
            None,
            "not a whole"
        );
        assert_eq!(receipt.status(), RequestStatus::Receiving);
        assert_eq!(
            link.note_request_resource_outcome(hash(1), &inbound_failed()),
            Some(hashes(2, 1)),
            "the failed entry names the segment in flight and the whole"
        );
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert!(link.pending_response_limits.is_empty(), "the limit went with the entry");
    }

    #[test]
    fn failing_entries_hands_back_the_segments_they_were_receiving_and_drops_their_limits() {
        let (mut link, _, _, _) = linked_pair();
        let first = pending_packet_request(&mut link, id(1), TIMEOUT);
        let second = pending_packet_request(&mut link, id(2), TIMEOUT);
        let third = pending_packet_request(&mut link, id(3), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(1), hash(1)));
        link.mark_pending_request_receiving(id(2).as_bytes(), &response_segment(hash(2), hash(2)));

        assert_eq!(link.fail_pending_request(&id(3), RequestError::Timeout), None);
        assert_eq!(link.fail_pending_request(&id(9), RequestError::Timeout), None, "unknown id");
        assert_eq!(third.status(), RequestStatus::Failed);
        assert_eq!(link.pending_response_limits.len(), 2);

        let mut receiving = link.fail_all_pending_requests(RequestError::LinkClosed);
        receiving.sort_by_key(|hashes| hashes.segment.to_bytes());
        assert_eq!(receiving, vec![hashes(1, 1), hashes(2, 2)]);
        assert_eq!(first.status(), RequestStatus::Failed);
        assert_eq!(second.status(), RequestStatus::Failed);
        assert!(link.pending_response_limits.is_empty());
    }

    #[test]
    fn a_displaced_entry_fails_send_and_hands_back_its_segment() {
        let (mut link, _, _, _) = linked_pair();
        let displaced = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(5), hash(5)));

        let (replacement, cancel) =
            link.insert_pending_request(id(1), PendingRequest::new(id(1), TIMEOUT, None));
        assert_eq!(cancel, Some(hashes(5, 5)));
        assert_eq!(displaced.status(), RequestStatus::Failed);
        assert_eq!(replacement.status(), RequestStatus::Sent);
        assert_eq!(link.fail_pending_request(&id(1), RequestError::Timeout), None);
        assert_eq!(replacement.status(), RequestStatus::Failed);
    }

    #[test]
    fn restart_and_a_fresh_link_request_drain_the_table_and_the_limits() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.restart();
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert!(link.pending_requests.is_empty());
        assert!(link.pending_response_limits.is_empty());

        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.request();
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert!(link.pending_requests.is_empty());
        assert!(link.pending_response_limits.is_empty());
    }

    #[test]
    fn a_closing_link_fails_every_entry_and_drops_the_limits() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(1), hash(1)));
        link.close();
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert!(link.pending_response_limits.is_empty());
    }

    /// A response that is not an envelope fails its receipt `Decode` through the link
    /// (`RNS/Link.py:857-884` would raise out of the unpack); the entry and its limit go
    /// with it, so a second copy of the response finds nothing waiting.
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn a_response_that_does_not_decode_fails_its_receipt_decode_and_drops_the_entry() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 1), Some(true));

        let receiving = link.resolve_pending_request(
            id(1),
            ResponseArrival::Resource { data: &[0xc0], metadata: None },
        );
        assert!(receiving.is_none());
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert_eq!(receipt.response().await, Err(RequestError::Decode));
        assert!(link.pending_requests.is_empty());
        assert_eq!(link.response_size_allowed(id(1).as_bytes(), 1), None, "the limit went with it");
        let receiving = link.resolve_pending_request(
            id(1),
            ResponseArrival::Resource { data: &[0xc0], metadata: None },
        );
        assert!(receiving.is_none(), "nothing was pending");
        assert_eq!(
            receipt.response().await,
            Err(RequestError::Decode),
            "a second copy changes nothing"
        );
    }

    /// The response size limit stays in force while a response Resource is inbound, so an
    /// over-limit response packet arriving then still fails the receipt `Oversize`
    /// (`RNS/Link.py:874-876`); the Resource it was receiving is owed a cancel
    /// (`RNS/Link.py:1435`), handed to the transport once.
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn an_oversize_response_packet_for_a_receiving_entry_fails_it_and_owes_a_cancel() {
        let (mut link, peer, iface, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1), TIMEOUT);
        link.set_response_size_limit(id(1).as_bytes(), 64);
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(2), hash(1)));
        assert_eq!(receipt.status(), RequestStatus::Receiving);
        assert!(link.take_owed_response_cancels().is_empty(), "nothing owed yet");

        let envelope = ResponseFrame::new(id(1), rmpv::Value::Binary(vec![0u8; 65])).encode();
        let packet = peer.response_packet(&envelope).expect("the peer builds the response");
        assert!(matches!(link.handle_packet(&packet, iface), LinkHandleResult::None));

        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert_eq!(receipt.response().await, Err(RequestError::Oversize));
        assert!(link.pending_requests.is_empty());
        assert!(link.pending_response_limits.is_empty(), "the limit went with the entry");
        assert_eq!(
            link.take_owed_response_cancels(),
            vec![hashes(2, 1)],
            "the segment in flight and the whole are owed a cancel"
        );
        assert!(link.take_owed_response_cancels().is_empty(), "handed over once");
    }

    /// `Transport::cancel_resource` on the request Resource (`RNS/Link.py:1377-1385`
    /// fails the receipt for every status but complete, a cancelled one included).
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn a_locally_cancelled_request_resource_fails_its_receipt_send() {
        let (mut link, _, _, _) = linked_pair();
        let cancelled = pending_resource_request(&mut link, id(1), hash(0xAA));
        let other = pending_resource_request(&mut link, id(2), hash(0xBB));

        assert_eq!(
            link.note_request_resource_outcome(hash(0xCC), &ResourceEventKind::OutboundCancelled),
            None,
            "no entry sent that Resource"
        );
        assert_eq!(other.status(), RequestStatus::Sent);

        assert_eq!(
            link.note_request_resource_outcome(hash(0xAA), &ResourceEventKind::OutboundCancelled),
            None,
            "nothing inbound to cancel"
        );
        assert_eq!(cancelled.status(), RequestStatus::Failed);
        assert_eq!(cancelled.response().await, Err(RequestError::Send));
        assert_eq!(other.status(), RequestStatus::Sent, "the other entry is untouched");
    }

    /// The request Resource stays cancellable until its proof arrives, so a
    /// response advertised ahead of a late proof is already inbound when the
    /// cancel fails the receipt; the segment comes back for the caller to cancel.
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn cancelling_a_request_resource_already_receiving_hands_back_the_segment() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_resource_request(&mut link, id(1), hash(0xAA));
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(2), hash(1)));
        assert_eq!(receipt.status(), RequestStatus::Receiving);

        assert_eq!(
            link.note_request_resource_outcome(hash(0xAA), &ResourceEventKind::OutboundCancelled),
            Some(hashes(2, 1)),
            "the failed entry names the segment in flight and the whole"
        );
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert_eq!(receipt.response().await, Err(RequestError::Send));
    }
}
