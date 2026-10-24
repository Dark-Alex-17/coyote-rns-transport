/// Python sends the LINKCLOSE for every status but `PENDING`
/// (`RNS/Link.py:668`); a responder that has derived its session cipher
/// but not yet proved is `Handshake`, so its teardown reaches the wire.
#[test]
fn teardown_sends_link_close_for_a_handshake_responder() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(4);

    let mut outbound = Link::new(destination, tx.clone());
    let request = outbound.request();
    let mut inbound = Link::new_from_request(&request, signer.sign_key().clone(), destination, tx)
        .expect("link request should parse");
    assert_eq!(inbound.status(), LinkStatus::Handshake);

    let packet = inbound.teardown().expect("handshake teardown should produce close packet");
    assert_eq!(packet.header.packet_type, PacketType::Data);
    assert_eq!(packet.context, PacketContext::LinkClose);
    assert_eq!(packet.destination, *inbound.id());
    assert_eq!(inbound.status(), LinkStatus::Closed);
}

#[test]
fn teardown_of_a_pending_initiator_sends_nothing() {
    let identity = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(4);

    let mut pending = Link::new(destination, tx);
    pending.request();
    assert_eq!(pending.status(), LinkStatus::Pending);

    assert!(pending.teardown().is_none());
    assert_eq!(pending.status(), LinkStatus::Closed);
}
