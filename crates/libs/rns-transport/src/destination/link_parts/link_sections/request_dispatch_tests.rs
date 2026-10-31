// Kept separate from `identify_packet_tests.rs` so that file remains below the
// repository's 500-line module limit.
#[cfg(test)]
mod request_dispatch_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    use tokio::sync::mpsc::error::TryRecvError;

    fn request_event(rx: &mut tokio::sync::broadcast::Receiver<LinkEventData>) -> LinkPayload {
        let event = rx.try_recv().expect("the request is posted to the link's event stream");
        match event.event {
            LinkEvent::Data(payload) => {
                assert_eq!(payload.context(), PacketContext::Request);
                *payload
            }
            _ => panic!("expected the request to arrive as LinkEvent::Data"),
        }
    }

    /// `if self.status == Link.ACTIVE` (`RNS/Link.py:805`): the dispatch
    /// queue only sees requests from an active link. The link's event stream
    /// still carries the request either way, because the gate sits in front
    /// of the queue, not in front of decryption. `Handshake` is the inactive
    /// state that keeps the session cipher, so the packet still decrypts and
    /// the gate alone decides; `Stale` would be revived by the inbound packet
    /// and `Closed` drops the cipher.
    #[test]
    fn only_an_active_link_queues_a_request_for_dispatch() {
        let (outbound, mut inbound, iface, mut rx) = linked_pair();
        let (tx, mut dispatch_rx) = tokio::sync::mpsc::channel(4);
        inbound.set_request_dispatch(tx);
        let request = b"\x93\xcb\x00\x00\x00\x00\x00\x00\x00\x00\xc4\x04path\xc0";

        let packet = outbound.request_packet(request).expect("a request packet can be built");
        inbound.handle_packet(&packet, iface);
        let item = dispatch_rx.try_recv().expect("an active link queues the request");
        let DispatchForm::Packet { request_id } = item.form else {
            panic!("a request packet is queued in packet form");
        };
        assert_eq!(request_id, RequestId::from_packet_hash(&packet.hash()));
        assert_eq!(item.plain_text, request);
        assert_eq!(item.link_id, *inbound.id());
        assert_eq!(item.destination_hash, inbound.destination().address_hash);
        assert!(item.remote_identity.is_none(), "nobody identified on this link");
        let posted = request_event(&mut rx);
        assert_eq!(posted.as_slice(), request);
        assert_eq!(posted.packet_hash(), Some(packet.hash()));

        inbound.status = LinkStatus::Handshake;
        let packet = outbound.request_packet(request).expect("a request packet can be built");
        inbound.handle_packet(&packet, iface);
        assert!(
            matches!(dispatch_rx.try_recv(), Err(TryRecvError::Empty)),
            "an inactive link never queues the request"
        );
        assert_eq!(request_event(&mut rx).as_slice(), request);
        assert_eq!(inbound.status, LinkStatus::Handshake, "a request does not activate a link");
    }
}
