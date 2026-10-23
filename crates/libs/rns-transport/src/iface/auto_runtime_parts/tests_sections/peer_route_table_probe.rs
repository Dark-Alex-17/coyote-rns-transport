// When the discovery and data tasks both miss the first lookup for one peer,
// the first registration to reach the table wins; before the table became the
// authority the peer received every broadcast twice. The sibling pin in
// `peer_route_table_expiry.rs` asserts the table shape; this one asserts what
// the peer sees, through the two seams the runtime drives
// (`register_discovered_peer` from the discovery task, `forward_peer_data`
// from the data task), by counting datagrams at a Python-shaped peer.

// A peer first heard on both sockets at once receives one copy of a
// broadcast, not one per registration.
#[tokio::test]
async fn peer_first_heard_on_both_sockets_at_once_receives_each_broadcast_once() {
    let peer = PythonShapedPeer::bind().await;
    let plan = loopback_plan_with_data_port(peer.data_port);
    let mut state = plan.discovery_state();
    let mut dedupe = AutoInboundPacketDeduplicator::from_timing(AutoInterfaceTiming::for_platform(
        AutoInterfacePlatform::Other,
    ));

    let iface_manager = Arc::new(tokio::sync::Mutex::new(InterfaceManager::new(8)));
    let _rx_recv = iface_manager.lock().await.receiver();
    let channel = iface_manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast);
    let runtime = AutoInterfaceTransportRuntime::from_channel(channel, Arc::clone(&iface_manager));
    let (bridge, tx_channel) = runtime.split();
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let _tx_handle =
        plan.spawn_peer_data_transport_tx_loop(bridge.clone(), tx_channel, shutdown_rx);
    let our_data_socket =
        Arc::new(tokio::net::UdpSocket::bind("127.0.0.1:0").await.expect("bind our data socket"));
    let our_bind_addr = our_data_socket.local_addr().expect("our data addr");

    // The peering announce and the first data datagram, both already
    // admitted, exactly what the two supervisor tasks hold when they race.
    let admitted = admit_peer(&plan, &mut state, peer.ephemeral_source());
    let inbound = test_packet(b"peer-to-us", 0x66);
    let processed = plan
        .process_peer_data_datagram(
            &mut state,
            &mut dedupe,
            AutoPeerDataDatagram {
                ifname: "lo".to_string(),
                bind_addr: our_bind_addr,
                source_addr: peer.ephemeral_source(),
                payload: inbound.to_bytes().expect("serialize inbound"),
            },
            core::time::Duration::from_millis(10),
        )
        .expect("peer data admitted");
    assert!(matches!(processed.decision, AutoPeerInboundDecision::Accepted { .. }));

    // Holding the manager lets both seams miss the first table lookup before
    // either can register its virtual iface.
    let gate = iface_manager.lock().await;
    let discovery = {
        let bridge = bridge.clone();
        let config = plan.config.clone();
        let socket = Arc::clone(&our_data_socket);
        tokio::spawn(
            async move { bridge.register_discovered_peer(&config, &admitted, socket).await },
        )
    };
    let data = {
        let bridge = bridge.clone();
        let socket = Arc::clone(&our_data_socket);
        let data_port = peer.data_port;
        tokio::spawn(async move { bridge.forward_peer_data(&processed, socket, data_port).await })
    };
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    drop(gate);
    let discovered =
        discovery.await.expect("discovery task").expect("discovery registers the peer");
    assert_eq!(data.await.expect("data task"), AutoPeerDataForwardResult::Delivered);

    let announce = test_packet(b"our-announce", 0x77);
    iface_manager
        .lock()
        .await
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: announce.clone() })
        .await;

    assert_eq!(
        peer.recv().await.as_ref(),
        Some(&announce),
        "the peer must still be reachable after the racing registrations"
    );
    assert_eq!(
        peer.recv().await,
        None,
        "a broadcast must reach a peer once, not once per racing registration"
    );
    let hashes = iface_manager.lock().await.interface_hashes();
    assert_eq!(hashes.len(), 2, "host plus exactly one virtual peer iface: {hashes:?}");
    assert!(hashes.contains(&discovered));
}
