// The response record no receipt owns, across `response_packet.rs` and
// `pending_requests.rs`. Kept separate from `pending_requests_tests.rs` so
// each stays below the repository's 500-line limit. `include!`d into the
// same module.
#[cfg(test)]
mod response_record_tests {
    use super::identify_packet_tests::linked_pair;
    use super::pending_requests_tests::{hash, hashes, response_segment};
    use super::*;

    /// A re-send under an id whose record is receiving its response, or whose request
    /// Resource is still in flight, does not restart a clock the record does not run.
    #[test]
    fn a_re_sent_request_packet_re_arms_only_a_running_clock() {
        let (mut link, _, _, _) = linked_pair();
        link.expect_response(b"legacy", Some(10));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));
        link.expect_response_to_resource(b"resource", None, hash(0xA1));

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
        link.expect_response_to_resource(b"legacy", None, hash(0xA1));
        link.mark_pending_request_receiving(b"legacy", &response_segment(hash(1), hash(3)));

        let rejected = ResourceEventKind::OutboundRejected;
        assert_eq!(link.note_request_resource_outcome(hash(0xA1), &rejected), Some(hashes(1, 3)));
        assert_eq!(link.response_size_allowed(b"legacy", 1), None, "dropped with the request");
    }
}
