// Kept separate from `pending_requests_tests.rs` so that file stays below
// the repository's 500-line module limit. `include!`d into the same module.
#[cfg(test)]
mod pending_requests_resolve_tests {
    use super::identify_packet_tests::linked_pair;
    use super::pending_requests_tests::{hash, hashes, id, response_segment};
    use super::*;

    use crate::destination::request::{RequestStatus, ResponseFrame};

    const TIMEOUT: Duration = Duration::from_secs(5);

    /// A packet-form request with a response size limit registered.
    fn pending_packet_request(link: &mut Link, id: RequestId) -> RequestReceipt {
        link.set_response_size_limit(id.as_bytes(), 1024);
        link.insert_pending_request(id, PendingRequest::new(id, TIMEOUT, None)).0
    }

    /// An in-limit response packet for a receipt whose response Resource is
    /// still inbound resolves the receipt `Ready` from the packet, as Python
    /// does (`RNS/Link.py:873`), and owes a cancel for that Resource, which
    /// Python lets run to completion and then discards (`RNS/Link.py:897-912`).
    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn an_in_limit_response_packet_for_a_receiving_entry_resolves_it_and_owes_a_cancel() {
        let (mut link, peer, iface, _) = linked_pair();
        let receipt = pending_packet_request(&mut link, id(1));
        link.mark_pending_request_receiving(id(1).as_bytes(), &response_segment(hash(2), hash(1)));
        assert_eq!(receipt.status(), RequestStatus::Receiving);
        assert!(link.take_owed_response_cancels().is_empty(), "nothing owed yet");

        let envelope = ResponseFrame::new(id(1), rmpv::Value::Binary(vec![7u8; 4])).encode();
        let packet = peer.response_packet(&envelope).expect("the peer builds the response");
        assert!(matches!(link.handle_packet(&packet, iface), LinkHandleResult::None));

        assert_eq!(receipt.status(), RequestStatus::Ready);
        let response = receipt.response().await.expect("the packet answered the request");
        assert_eq!(response.value(), Some(&rmpv::Value::Binary(vec![7u8; 4])));
        assert!(link.pending_requests.is_empty());
        assert!(link.pending_response_limits.is_empty(), "the limit went with the entry");
        assert_eq!(
            link.take_owed_response_cancels(),
            vec![hashes(2, 1)],
            "the response Resource still in flight is owed a cancel"
        );
        assert!(link.take_owed_response_cancels().is_empty(), "handed over once");
    }
}
