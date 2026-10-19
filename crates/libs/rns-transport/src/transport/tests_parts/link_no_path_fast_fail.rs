// `Transport::link` toward a destination the path table does not know. Without broadcast
// the request has nowhere to go and `link` refuses it at once instead of letting the link
// time out; with broadcast it still goes out on every interface, as `RNS/Transport.py:1443`
// floods a packet without a path entry.

fn unknown_remote_destination() -> DestinationDesc {
    let remote_identity = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    DestinationDesc {
        identity: remote_identity,
        address_hash: remote_identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn link_without_path_and_without_broadcast_fails_fast() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("link-fast-fail", &identity, false));
    let mut channel =
        transport.iface_manager().lock().await.new_channel_with_role(8, IfaceRole::Unicast);
    let destination = unknown_remote_destination();

    let started = std::time::Instant::now();
    let result = transport.link(destination).await;

    assert!(matches!(result, Err(RnsError::NoPath)), "link request must be refused");
    assert!(started.elapsed() < Duration::from_millis(10), "took {:?}", started.elapsed());
    assert!(
        matches!(channel.tx_channel.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
        "no link request on the wire"
    );
    assert!(!transport
        .get_handler()
        .lock()
        .await
        .out_links
        .contains_key(&destination.address_hash));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn link_without_path_with_broadcast_still_sends_the_request() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("link-broadcast", &identity, true));
    let mut channel =
        transport.iface_manager().lock().await.new_channel_with_role(8, IfaceRole::Unicast);
    let destination = unknown_remote_destination();

    let link = transport.link(destination).await.expect("link request sent");
    assert_eq!(link.lock().await.status(), LinkStatus::Pending);

    let request = timeout(Duration::from_secs(2), channel.tx_channel.recv())
        .await
        .expect("link request within the timeout")
        .expect("channel open");
    assert_eq!(request.packet.header.packet_type, PacketType::LinkRequest);
}
