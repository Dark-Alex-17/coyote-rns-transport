// Kept separate from `response_packet.rs` so each stays below the
// repository's 500-line limit. `include!`d into the same module.
#[cfg(test)]
mod response_packet_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    fn id(byte: u8) -> RequestId {
        RequestId::new([byte; ADDRESS_HASH_SIZE])
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
}
