// The response record no receipt owns, across `response_packet.rs` and
// `pending_requests.rs`. Kept separate from `pending_requests_tests.rs` so
// each stays below the repository's 500-line limit. `include!`d into the
// same module.
#[cfg(test)]
mod response_record_tests {
    use super::identify_packet_tests::linked_pair;
    use super::pending_requests_tests::{hash, hashes, id, response_segment};
    use super::*;

    use crate::destination::request::{RequestStatus, ResponseFrame};

    /// A re-send under an id whose record is receiving its response, or whose request
    /// Resource is still in flight, does not restart a clock the record does not run.
    #[test]
    fn a_re_sent_request_packet_re_arms_only_a_running_clock() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", Some(10));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));
        assert_eq!(link.expect_response_to_resource(b"resource", None, hash(0xA1)), None);

        link.note_request_sent(b"legacy");
        link.note_request_sent(b"resource");
        let entry = link.pending_response_limits.get(b"legacy".as_slice()).expect("recorded");
        assert_eq!(entry.clock, RecordClock::Receiving(hashes(1, 3)), "still receiving");
        assert_eq!(entry.limit, Some(10), "the send keeps the limit");
        let entry = link.pending_response_limits.get(b"resource".as_slice()).expect("recorded");
        assert_eq!(entry.clock, RecordClock::Unarmed, "still waiting on the request Resource");
        assert_eq!(link.earliest_pending_request_deadline(), None, "no clock was started");
    }

    /// Taking a record hands back the response Resource it was receiving, once; a record
    /// not receiving anything hands back nothing.
    #[test]
    fn taking_a_receiving_record_returns_its_inbound_response_once() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", None);
        link.expect_response(b"idle", None);
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        assert_eq!(link.take_response_record(b"legacy"), Some(hashes(1, 3)));
        assert_eq!(link.take_response_record(b"legacy"), None, "the record is gone");
        assert_eq!(link.response_size_allowed(b"legacy", 1), None);
        assert_eq!(link.take_response_record(b"idle"), None, "nothing inbound to cancel");
        assert_eq!(link.response_size_allowed(b"idle", 1), None, "removed all the same");
    }

    /// A request Resource rejected after a response segment was already admitted drops the
    /// entry and hands back the response it was receiving, for the caller to cancel.
    #[test]
    fn a_rejected_request_resource_hands_back_the_response_its_entry_was_receiving() {
        let (mut link, _, _, _) = linked_pair();
        assert_eq!(link.expect_response_to_resource(b"legacy", None, hash(0xA1)), None);
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        let rejected = ResourceEventKind::OutboundRejected;
        assert_eq!(link.note_request_resource_outcome(hash(0xA1), &rejected), Some(hashes(1, 3)));
        assert_eq!(link.response_size_allowed(b"legacy", 1), None, "dropped with the request");
    }

    /// An over-limit response packet for a receipt-less record that is receiving a response
    /// Resource refuses the packet, drops the record, and owes that Resource a cancel once;
    /// there is no receipt to supply it.
    #[test]
    fn an_oversize_packet_for_a_receipt_less_receiving_record_owes_its_resource_a_cancel() {
        let (mut link, _, _, _) = linked_pair();
        let request = id(7);
        let q = request.as_bytes().as_slice();
        link.expect_response(q, Some(10));
        link.mark_pending_request_receiving(q, &response_segment(hash(1), hash(3)));
        assert!(link.pending_requests.is_empty(), "no receipt owns the id");

        assert!(!link.take_response_limit_if_allowed(q, 11), "over the limit");
        assert_eq!(link.response_size_allowed(q, 1), None, "the record is gone");
        assert_eq!(link.take_owed_response_cancels(), vec![hashes(1, 3)]);
        assert!(link.take_owed_response_cancels().is_empty(), "handed over once");
    }

    /// The crate-private writer hands the receiving Resource back only when it refuses the
    /// packet; an admitted packet leaves the inbound Resource to run, as a receipt does.
    #[test]
    fn take_response_limit_hands_back_the_receiving_resource_only_on_refusal() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"refused", Some(10));
        link.mark_pending_request_receiving(b"refused", &response_segment(hash(1), hash(3)));
        link.expect_response(b"admitted", Some(10));
        link.mark_pending_request_receiving(b"admitted", &response_segment(hash(2), hash(4)));

        assert_eq!(link.take_response_limit(b"refused", 11), (false, Some(hashes(1, 3))));
        assert_eq!(link.take_response_limit(b"admitted", 10), (true, None));
        assert!(link.take_owed_response_cancels().is_empty(), "the caller routes the hashes");
        assert_eq!(link.response_size_allowed(b"admitted", 1), None, "removed all the same");
    }

    /// The over-limit response packet itself, handled by the link under a receipt-less
    /// 16-byte id whose record is receiving a response Resource: the packet is refused and
    /// exactly one cancel is owed, where a receipt-owned id gets its one from the receipt
    /// (see `pending_requests_tests`).
    #[test]
    fn handling_an_oversize_packet_under_a_receipt_less_receiving_record_owes_one_cancel() {
        let (mut link, peer, iface, _) = linked_pair();
        let request = id(7);
        let q = request.as_bytes().as_slice();
        link.expect_response(q, Some(64));
        link.mark_pending_request_receiving(q, &response_segment(hash(2), hash(1)));

        let envelope = ResponseFrame::new(request, rmpv::Value::Binary(vec![0u8; 65])).encode();
        let packet = peer.response_packet(&envelope).expect("the peer builds the response");
        assert!(matches!(link.handle_packet(&packet, iface), LinkHandleResult::None));

        assert!(link.pending_response_limits.is_empty(), "the record went with the refusal");
        assert_eq!(link.take_owed_response_cancels(), vec![hashes(2, 1)]);
        assert!(link.take_owed_response_cancels().is_empty(), "handed over once");
    }

    /// Replacing a record whole hands back the response Resource it was receiving; an idle
    /// record hands back nothing.
    #[test]
    fn replacing_a_receiving_record_hands_back_its_inbound_response() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", None);
        link.expect_response(b"idle", None);
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        assert_eq!(link.replace_response_record(b"legacy", Some(10)), Some(hashes(1, 3)));
        assert_eq!(link.replace_response_record(b"idle", None), None);
        assert_eq!(link.response_size_limit(b"legacy"), Some(10), "replaced, not removed");
        assert!(link.take_owed_response_cancels().is_empty(), "the caller routes the hashes");
    }

    /// A request Resource re-sent under an id whose record is receiving a response hands
    /// back that response for the caller to cancel.
    #[test]
    fn expecting_a_response_to_a_resource_over_a_receiving_record_hands_back_its_response() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", None);
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        assert_eq!(
            link.expect_response_to_resource(b"legacy", None, hash(0xA1)),
            Some(hashes(1, 3))
        );
        let entry = link.pending_response_limits.get(b"legacy".as_slice()).expect("recorded");
        assert_eq!(entry.clock, RecordClock::Unarmed, "waiting on the new request Resource");
        assert_eq!(link.expect_response_to_resource(b"legacy", None, hash(0xA2)), None);
    }

    /// The public `expect_response` over a receiving record owes the displaced response one
    /// cancel, for the transport to send on the link's next inbound packet, Resource packets
    /// included.
    #[test]
    fn expecting_a_response_over_a_receiving_record_owes_the_displaced_response_a_cancel() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", Some(10));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        link.expect_response(b"legacy", None);
        assert_eq!(link.response_size_limit(b"legacy"), Some(10), "the limit stays");
        assert_eq!(link.take_owed_response_cancels(), vec![hashes(1, 3)]);
        assert!(link.take_owed_response_cancels().is_empty(), "handed over once");
    }

    /// Failing a 16-byte id no receipt owns hands back the response Resource its record was
    /// receiving, once, and drops the record; an idle record is dropped handing back nothing.
    #[test]
    fn failing_a_receipt_less_receiving_record_hands_back_its_inbound_response() {
        let (mut link, _, _, _) = linked_pair();
        let receiving = id(7);
        let q = receiving.as_bytes().as_slice();
        link.expect_response(q, Some(10));
        link.mark_pending_request_receiving(q, &response_segment(hash(1), hash(3)));
        let idle = id(8);
        link.expect_response(idle.as_bytes(), Some(10));
        assert!(link.pending_requests.is_empty(), "no receipt owns either id");

        assert_eq!(
            link.fail_pending_request(&receiving, RequestError::Oversize),
            Some(hashes(1, 3))
        );
        assert_eq!(link.response_size_allowed(q, 1), None, "the record is gone");
        assert_eq!(link.fail_pending_request(&receiving, RequestError::Oversize), None, "once");
        assert_eq!(link.fail_pending_request(&idle, RequestError::Oversize), None);
        assert_eq!(link.response_size_allowed(idle.as_bytes(), 1), None, "removed all the same");
    }

    /// An assembled response over the limit of a 16-byte id no receipt owns fails the record
    /// and hands back what it was receiving; one under the limit resolves it, cancelling
    /// nothing. The limit goes either way.
    #[test]
    fn resolving_an_over_limit_response_for_a_receipt_less_receiving_record_hands_back_its_response(
    ) {
        let (mut link, _, _, _) = linked_pair();
        let request = id(7);
        let q = request.as_bytes().as_slice();
        link.expect_response(q, Some(10));
        link.mark_pending_request_receiving(q, &response_segment(hash(1), hash(3)));

        let over = ResponseArrival::Resource { data: &[0u8; 11], metadata: None };
        assert_eq!(link.resolve_pending_request(request, over), Some(hashes(1, 3)));
        assert_eq!(link.response_size_allowed(q, 1), None, "the record is gone");

        link.expect_response(q, Some(10));
        link.mark_pending_request_receiving(q, &response_segment(hash(2), hash(4)));
        let within = ResponseArrival::Resource { data: &[0u8; 10], metadata: None };
        assert_eq!(link.resolve_pending_request(request, within), None, "assembled, not inbound");
        assert_eq!(link.response_size_allowed(q, 1), None, "the limit went with the response");
    }

    /// A link closing or starting over drops the cancels it still owed with its state.
    #[test]
    fn abandoning_pending_requests_clears_the_owed_cancels() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", Some(10));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));
        link.expect_response(b"legacy", None);
        assert_eq!(link.owed_response_cancels, vec![hashes(1, 3)], "one cancel owed");

        link.abandon_pending_requests();
        assert!(link.take_owed_response_cancels().is_empty(), "dropped with the link state");
    }

    /// A fresh link request, a restart and a close drop every record no receipt owns and
    /// every cancel still owed, with the receipts: nothing of the old link outlives it.
    #[test]
    fn a_fresh_link_request_a_restart_and_a_close_drop_the_records_and_the_owed_cancels() {
        type Reset = fn(&mut Link);
        let resets: [(&str, Reset); 3] = [
            ("request", |link| {
                link.request();
            }),
            ("restart", Link::restart),
            ("close", Link::close),
        ];
        for (name, reset) in resets {
            let (mut link, _, _, _) = linked_pair();
            link.note_request_sent(b"bare");
            link.expect_response(b"legacy", Some(10));
            link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));
            link.expect_response(b"legacy", None);
            assert_eq!(link.pending_response_limits.len(), 2, "{name}: two records seeded");
            assert!(!link.owed_response_cancels.is_empty(), "{name}: one cancel owed");

            reset(&mut link);
            assert!(link.pending_requests.is_empty(), "{name}: no receipt survives");
            assert!(link.pending_response_limits.is_empty(), "{name}: no record survives");
            assert!(link.take_owed_response_cancels().is_empty(), "{name}: nothing owed");
        }
    }

    /// A completed request Resource marks the entry that sent it `Delivered`
    /// (`RNS/Link.py:1372`) and nothing else: a packet-form entry has no request Resource
    /// to match, so it stays `Sent`, as does every entry when the hash matches none.
    #[test]
    fn a_request_resource_outcome_leaves_a_packet_form_entry_in_sent() {
        let (mut link, _, _, _) = linked_pair();
        let timeout = Duration::from_secs(30);
        let (packet_form, _) =
            link.insert_pending_request(id(1), PendingRequest::new(id(1), timeout, None));
        let (resource_form, _) = link
            .insert_pending_request(id(2), PendingRequest::new(id(2), timeout, Some(hash(0xAA))));

        let complete = ResourceEventKind::OutboundComplete;
        assert_eq!(link.note_request_resource_outcome(hash(0xAA), &complete), None);
        assert_eq!(packet_form.status(), RequestStatus::Sent, "no request Resource to prove");
        assert_eq!(resource_form.status(), RequestStatus::Delivered);

        assert_eq!(link.note_request_resource_outcome(hash(0xBB), &complete), None);
        assert_eq!(packet_form.status(), RequestStatus::Sent, "a hash matching nothing");
        assert_eq!(resource_form.status(), RequestStatus::Delivered);
    }
}
