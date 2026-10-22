// Pins the receipt guard across a shared-instance drop, seen the way a
// consumer sees it: a plain single-destination packet the client sent on a direct interface
// before the shared instance went away is still proved to the client's `ReceiptHandler`
// once the peer's proof arrives after the flush. Python's
// `shared_connection_disappeared` empties `reverse_table` (`RNS/Transport.py:3699`) but
// never `receipts` (`RNS/Transport.py:174`), while `path_table = {}` (`:3698`) takes every
// path with it, the direct peer's included. The peer sits on an in-memory interface so its
// proof is held back until the drop has been recorded. Helpers come from the sibling
// `tests_parts` files included into this module; the receipt recorder is the one
// `two_transport_loopback_link_packet_proven.rs` defines.

/// A peer reachable over an in-memory interface of the client, with a destination that
/// proves every packet it receives.
struct SharedInstanceProbe8Peer {
    transport: Transport,
    iface: crate::iface::InterfaceChannel,
    address_hash: AddressHash,
}

/// Brings the peer up and teaches `client` its path over a fresh in-memory interface,
/// returning the peer and the client's end of that interface.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe8_direct_peer(
    client: &Transport,
) -> (SharedInstanceProbe8Peer, crate::iface::InterfaceChannel) {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-probe8-peer", &identity, true));
    let iface = transport.iface_manager().lock().await.new_channel(8);
    let destination = transport
        .add_destination(identity, DestinationName::new("shared-instance-probe8", "proof"))
        .await;
    let address_hash = destination.lock().await.desc.address_hash;
    destination.lock().await.set_proof_strategy(ProofStrategy::All);
    let announce = destination.lock().await.announce(OsRng, None).expect("valid announce");

    let direct = client.iface_manager().lock().await.new_channel(8);
    handle_announce(
        &announce,
        client.get_handler().lock().await,
        direct.address,
        crate::iface::IfaceSource::None,
    )
    .await;
    assert!(
        client.has_path(&address_hash).await,
        "the client learned the peer over the direct iface"
    );
    (SharedInstanceProbe8Peer { transport, iface, address_hash }, direct)
}

/// The client sends a plain packet to the direct peer, the shared instance is restarted
/// before the peer has looked at it, and the proof the peer then returns still reaches the
/// client's `ReceiptHandler` with the sent hash and no link: the flush took the peer's path
/// and the proof routes of what the client had received, not the guard of what it sent.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_probe8_drop_keeps_the_receipt_of_a_packet_sent_on_another_iface() {
    install_test_logger();
    let name = "shared-instance-probe8-receipt";
    let server = shared_instance_server().await;
    let client = shared_instance_reconnect_client(name, &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;
    let (mut peer, mut direct) = shared_instance_probe8_direct_peer(&client.transport).await;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    client.transport.set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone()))).await;

    let outbound = Packet {
        destination: peer.address_hash,
        data: PacketDataBuffer::new_from_slice(b"proved after the drop"),
        ..Packet::default()
    };
    assert_eq!(
        client.transport.send_packet_with_trace(outbound).await.outcome,
        SendPacketOutcome::SentDirect,
        "the packet leaves on the direct iface"
    );
    let sent = timeout(LOOPBACK_TIMEOUT, direct.tx_channel.recv())
        .await
        .expect("packet queued on the direct iface")
        .expect("tx channel open");
    let sent_hash = sent.packet.hash();

    let (restarted, _restarted_announces, _) = shared_instance_reconnect_restart(server).await;
    wait_until(
        "the drop to be recorded",
        || !shared_instance_debug_records_containing(name, "disconnected,").is_empty(),
        || format!("{:?}", shared_instance_records_for(name)),
    )
    .await;
    assert!(
        !client.transport.has_path(&peer.address_hash).await,
        "the flush takes the direct peer's path too (RNS/Transport.py:3698)"
    );
    assert!(receipts.lock().expect("recording lock").is_empty(), "nothing is proved yet");

    handle_data(&sent.packet, peer.iface.address, peer.transport.get_handler().lock().await).await;
    let proof = timeout(LOOPBACK_TIMEOUT, peer.iface.tx_channel.recv())
        .await
        .expect("a delivery proof comes back")
        .expect("tx channel open");
    assert_eq!(proof.packet.header.packet_type, PacketType::Proof);
    client.transport.handle_inbound_for_test(proof.packet).await;

    let seen = receipts.lock().expect("recording lock").clone();
    assert_eq!(seen.len(), 1, "the proof still concludes the receipt after the flush: {seen:?}");
    assert_eq!(
        seen[0].packet_hash(),
        sent_hash,
        "the receipt names the packet sent before the drop"
    );
    assert_eq!(seen[0].link_id(), None, "a single-destination proof names no link");
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, peer.transport.stop()).await.expect("peer stop completes");
    timeout(STOP_TIMEOUT, restarted.transport.stop())
        .await
        .expect("restarted shared instance stop completes");
}
