// Kept separate from `response_packet.rs` so each stays below the
// repository's 500-line limit. `include!`d into the same module.
#[cfg(test)]
mod response_packet_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    fn id(byte: u8) -> RequestId {
        RequestId::new([byte; ADDRESS_HASH_SIZE])
    }

    fn hash(byte: u8) -> Hash {
        Hash::new_from_slice(&[byte; HASH_SIZE])
    }

    /// A receipt-less entry under a 16-byte id reports its limit, so the wire can measure
    /// an assembled response against it; there is no receipt to fail, and the entry goes
    /// when the measurement refuses the response.
    #[test]
    fn a_receipt_less_sixteen_byte_entry_reports_its_limit() {
        let (mut link, _, _, _) = linked_pair();
        let request = id(1);
        let q = request.as_bytes().as_slice();
        link.expect_response(q, Some(512));
        assert!(link.pending_requests.is_empty(), "no receipt owns the id");
        assert_eq!(link.response_size_limit(q), Some(512));
        assert!(!link.take_response_limit_if_allowed(q, 513), "assembled over");
        assert_eq!(link.response_size_limit(q), None, "and the entry went with it");
    }

    /// `set_response_size_limit` and the record a request makes compose in either order:
    /// the limit survives a record made after it without one, and set on a record already
    /// made it leaves the clock and the request Resource as they were.
    #[test]
    fn set_response_size_limit_composes_with_the_request_record_in_either_order() {
        let (mut link, _, _, _) = linked_pair();

        link.set_response_size_limit(b"limit-first", 512);
        link.expect_response_to_resource(b"limit-first", None, hash(1));
        assert_eq!(link.response_size_limit(b"limit-first"), Some(512));
        let entry = link.pending_response_limits.get(b"limit-first".as_slice()).expect("recorded");
        assert_eq!(entry.clock, RecordClock::Unarmed, "the request Resource arms the clock");
        assert_eq!(entry.request_resource, Some(hash(1)));

        link.expect_response_to_resource(b"record-first", None, hash(2));
        link.set_response_size_limit(b"record-first", 256);
        assert_eq!(link.response_size_limit(b"record-first"), Some(256));
        let entry = link.pending_response_limits.get(b"record-first".as_slice()).expect("kept");
        assert_eq!(entry.clock, RecordClock::Unarmed, "the limit does not start the clock");
        assert_eq!(entry.request_resource, Some(hash(2)));
    }

    /// A record made without a limit keeps one recorded before it; the writer behind
    /// `Transport::request` replaces the record whole, limit included, so a request sent
    /// without a limit under a displaced id does not inherit the stale one.
    #[test]
    fn expect_response_keeps_a_prior_limit_and_the_transport_writer_replaces_it() {
        let (mut link, _, _, _) = linked_pair();
        let request = id(1);
        let q = request.as_bytes().as_slice();

        link.expect_response(q, Some(512));
        link.expect_response(q, None);
        assert_eq!(link.response_size_limit(q), Some(512), "the earlier limit stays");
        assert!(
            matches!(
                link.pending_response_limits.get(q).map(|entry| entry.clock),
                Some(RecordClock::Expires(_))
            ),
            "a packet-form record runs from now"
        );

        link.replace_response_record(q, None);
        assert_eq!(link.response_size_limit(q), None, "replaced whole");
    }
}
