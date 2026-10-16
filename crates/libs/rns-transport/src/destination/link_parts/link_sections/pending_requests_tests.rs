// Kept separate from `pending_requests.rs` so that file stays below the
// repository's 500-line module limit.
#[cfg(test)]
mod pending_requests_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    use crate::destination::request::{RequestStatus, ResponseFrame};
    use crate::resource::{ResourceFailure, ResourceProgress, RANDOM_HASH_SIZE};

    const TIMEOUT: Duration = Duration::from_secs(5);

    fn id(byte: u8) -> RequestId {
        RequestId::new([byte; ADDRESS_HASH_SIZE])
    }

    fn hash(byte: u8) -> Hash {
        Hash::new_from_slice(&[byte; HASH_SIZE])
    }

    fn hashes(segment: u8, original: u8) -> InboundResponseHashes {
        InboundResponseHashes { segment: hash(segment), original: hash(original) }
    }

    fn response_segment(segment_hash: Hash, original: Hash) -> ResourceAdvertisement {
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
    fn pending_resource_request(link: &mut Link, id: RequestId, request_hash: Hash) -> RequestReceipt {
        link.insert_pending_request(id, PendingRequest::new(id, TIMEOUT, Some(request_hash))).0
    }

    /// `RNS/Link.py:1046-1049` reads the gate at every segment advertisement: an
    /// advertisement the ResourceManager drops leaves the limit where it was, the next one
    /// is measured against it, a receipt without a limit admits any size, an id nobody
    /// waits for solicits nothing, and a `send_request_resource*` id of any length is
    /// solicited by its entry alone. The limit goes when the response resolves.
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

        link.expect_response(b"legacy", None);
        assert_eq!(link.response_size_allowed(b"legacy", usize::MAX), Some(true));
        link.expect_response(b"legacy", Some(10));
        assert_eq!(link.response_size_allowed(b"legacy", 11), Some(false));
        link.clear_response_size_limit(b"legacy");
        assert_eq!(link.response_size_allowed(b"legacy", 1), None);

        let envelope = ResponseFrame::new(id(1), rmpv::Value::from(7)).encode();
        link.resolve_pending_request(
            id(1),
            ResponseArrival::Resource { data: &envelope, metadata: None },
        );
        assert_eq!(bounded.status(), RequestStatus::Ready);
        assert_eq!(link.response_size_allowed(q, 1), None, "the limit went with the entry");
    }

    #[test]
    fn a_receiving_entry_is_not_expired_and_not_folded_into_the_watchdog() {
        let (mut link, _, _, _) = linked_pair();
        let receiving = pending_packet_request(&mut link, id(1), Duration::ZERO);
        let waiting = pending_packet_request(&mut link, id(2), TIMEOUT);
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(1), hash(1)));
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

    #[test]
    fn a_resource_form_entry_expires_from_delivered_not_from_send() {
        let (mut link, _, _, _) = linked_pair();
        let receipt = pending_resource_request(&mut link, id(1), hash(0xA1));
        assert_eq!(link.earliest_pending_request_deadline(), None);
        link.expire_pending_requests(Instant::now() + TIMEOUT * 2);
        assert_eq!(receipt.status(), RequestStatus::Sent, "no clock before delivery");

        link.note_request_resource_outcome(hash(0xA1), &ResourceEventKind::OutboundComplete);
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
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(2), hash(1)));

        assert_eq!(link.note_request_resource_outcome(hash(2), &inbound_failed()), None, "not a whole");
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
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(1), hash(1)));
        link.mark_pending_request_receiving(&id(2), &response_segment(hash(2), hash(2)));

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
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(5), hash(5)));

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
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(1), hash(1)));
        link.close();
        assert_eq!(receipt.status(), RequestStatus::Failed);
        assert!(link.pending_response_limits.is_empty());
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
        link.mark_pending_request_receiving(&id(1), &response_segment(hash(2), hash(1)));
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
