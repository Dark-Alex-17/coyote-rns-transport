#[tokio::test]
async fn held_udp_announce_preserves_peer_source_for_unicast_route() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &local_identity, true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let mut announce_rx = transport.recv_announces().await;
    let iface = register_fake_multicast_iface(&transport).await;

    handler.lock().await.announce_limits = AnnounceLimits::with_rate_limit(AnnounceRateLimit {
        incoming_freq_samples: 3,
        max_held_announces: 8,
        new_time: Duration::from_secs(3600),
        burst_freq_new: 100.0,
        burst_freq: 100.0,
        burst_hold: Duration::from_millis(20),
        burst_penalty: Duration::from_millis(20),
        held_release_interval: Duration::from_millis(10),
    });

    let first_peer = peer_addr(4242);
    let mut first_destination = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "delivery"),
    );
    let first_announce = first_destination.announce(OsRng, None).expect("first announce");
    assert!(super::announce::admit_announce_before_queue(
        &first_announce,
        &handler,
        iface,
        crate::iface::IfaceSource::Udp(first_peer),
    )
    .await);
    handle_announce(
        &first_announce,
        handler.lock().await,
        iface,
        crate::iface::IfaceSource::Udp(first_peer),
    )
    .await;
    timeout(Duration::from_millis(200), announce_rx.recv())
        .await
        .expect("first announce should emit")
        .expect("broadcast receive");

    for idx in 0..3 {
        let mut packet = Packet::default();
        packet.header.packet_type = PacketType::Announce;
        packet.header.hops = 15;
        packet.destination = AddressHash::new([0xA0 + idx; crate::hash::ADDRESS_HASH_SIZE]);
        let _ = handler.lock().await.announce_limits.check(
            iface,
            &packet,
            crate::iface::IfaceSource::None,
            false,
        );
    }

    let held_peer = peer_addr(5252);
    let mut held_destination = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "delivery"),
    );
    let held_announce = held_destination.announce(OsRng, None).expect("held announce");
    assert!(!super::announce::admit_announce_before_queue(
        &held_announce,
        &handler,
        iface,
        crate::iface::IfaceSource::Udp(held_peer),
    )
    .await);
    assert!(matches!(
        announce_rx.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));

    let mut released = None;
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(30)).await;
        release_held_announces(handler.lock().await).await;
        if let Ok(event) = timeout(Duration::from_millis(80), announce_rx.recv()).await {
            released = Some(event.expect("broadcast receive"));
            break;
        }
    }
    let released = released.expect("held announce should release");

    let guard = handler.lock().await;
    let virtual_hash = guard
        .unicast_udp_ifaces
        .get(&held_peer)
        .map(|(hash, _)| *hash)
        .expect("held peer should register virtual iface");
    assert_eq!(released.interface, virtual_hash.as_slice().to_vec());

    let routing = guard.multicast_peer_routings.get(&iface).expect("routing").lock().await;
    assert_eq!(routing.hash_for_addr(&held_peer), Some(virtual_hash));
}

#[tokio::test]
async fn learned_announces_are_not_held_after_route_is_known() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &local_identity, true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let mut announce_rx = transport.recv_announces().await;

    handler.lock().await.announce_limits = AnnounceLimits::with_rate_limit(AnnounceRateLimit {
        incoming_freq_samples: 3,
        max_held_announces: 8,
        new_time: Duration::from_secs(3600),
        burst_freq_new: 100.0,
        burst_freq: 100.0,
        burst_hold: Duration::from_millis(20),
        burst_penalty: Duration::from_millis(20),
        held_release_interval: Duration::from_millis(10),
    });

    let iface = AddressHash::new_from_rand(OsRng);
    let mut destination = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "delivery"),
    );
    let announce = destination.announce(OsRng, None).expect("announce");

    handle_announce(&announce, handler.lock().await, iface, crate::iface::IfaceSource::None).await;
    timeout(Duration::from_millis(200), announce_rx.recv())
        .await
        .expect("first announce should emit")
        .expect("broadcast receive");

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let repeated_announce = destination.announce(OsRng, None).expect("fresh announce");
    handle_announce(
        &repeated_announce,
        handler.lock().await,
        iface,
        crate::iface::IfaceSource::None,
    )
    .await;

    let repeated = timeout(Duration::from_millis(200), announce_rx.recv())
        .await
        .expect("known announce should bypass ingress hold")
        .expect("broadcast receive");
    assert_eq!(repeated.hops, repeated_announce.header.hops);
}

#[tokio::test]
async fn fresh_equal_hop_announce_refreshes_known_path_iface() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("test", &local_identity, true);
    config.set_retransmit(true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();

    let (first_iface, second_iface) = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        (*manager.new_channel(16).address(), *manager.new_channel(16).address())
    };

    let mut destination = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "delivery"),
    );
    let first_announce = destination.announce(OsRng, None).expect("first announce");
    let announced_destination = first_announce.destination;
    handle_announce(
        &first_announce,
        handler.lock().await,
        first_iface,
        crate::iface::IfaceSource::None,
    )
    .await;
    assert_eq!(
        handler.lock().await.path_table.next_hop_iface(&announced_destination),
        Some(first_iface)
    );

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let second_announce = destination.announce(OsRng, None).expect("fresh announce");
    handle_announce(
        &second_announce,
        handler.lock().await,
        second_iface,
        crate::iface::IfaceSource::None,
    )
    .await;

    assert_eq!(
        handler.lock().await.path_table.next_hop_iface(&announced_destination),
        Some(second_iface),
        "fresh equal-hop announces should update an already-known route"
    );
}

#[tokio::test]
async fn path_response_announces_are_not_held_by_rate_limits() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &local_identity, true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let mut announce_rx = transport.recv_announces().await;

    handler.lock().await.announce_limits = AnnounceLimits::with_rate_limit(AnnounceRateLimit {
        incoming_freq_samples: 1,
        max_held_announces: 8,
        new_time: Duration::from_secs(3600),
        burst_freq_new: 0.0,
        burst_freq: 0.0,
        burst_hold: Duration::from_secs(60),
        burst_penalty: Duration::from_secs(60),
        held_release_interval: Duration::from_secs(60),
    });

    let iface = AddressHash::new_from_rand(OsRng);
    let mut destination = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("lxmf", "propagation"),
    );
    let mut announce = destination.announce(OsRng, None).expect("announce");
    announce.context = PacketContext::PathResponse;

    handle_announce(&announce, handler.lock().await, iface, crate::iface::IfaceSource::None).await;

    let received = timeout(Duration::from_millis(200), announce_rx.recv())
        .await
        .expect("path response announce should emit immediately")
        .expect("broadcast receive");
    assert_eq!(received.destination.lock().await.desc.address_hash, announce.destination);
}
