/// The host channel's `peer_count` tracks the discovery peer table once
/// `set_peer_counter` installs the state's counter: `Some(0)` before any
/// peer, `Some(1)` after one admitted discovery datagram, read the same
/// through a virtual child registered after the install, and `Some(0)`
/// again once the peer expires. A channel no counter reached reads `None`.
/// A status handle whose counter the state adopts, the direction the spawn
/// path takes, reads the same length.
#[test]
fn host_and_virtual_child_read_the_discovery_peer_table_length() {
    let plan = loopback_software_auto_plan(core::time::Duration::from_millis(50));
    // Any loopback address other than the adopted device's 127.0.0.2, so the
    // datagram is read as a remote peer and not as a local echo.
    let peer_addr: SocketAddr = "127.0.0.1:48556".parse().expect("software peer addr");
    let peer_address = peer_addr.ip().to_string();
    let discovery_datagram = AutoDiscoveryDatagram {
        kind: AutoDiscoverySocketKind::Unicast,
        ifname: "lo".to_string(),
        bind_addr: "127.0.0.1:0".parse().expect("software bind addr"),
        multicast_group_addr: None,
        source_addr: peer_addr,
        payload: crate::iface::auto::peering_token(plan.config.group_id.as_bytes(), &peer_address)
            .to_vec(),
    };

    let timing = AutoInterfaceTiming::for_platform(plan.platform);
    let peering_timeout = core::time::Duration::from_millis(200);
    let mut state = AutoDiscoveryState::new(
        plan.adopted_devices.clone(),
        peering_timeout,
        timing.reverse_peering_interval,
    );
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    assert_eq!(handle.peer_count(), 0);
    state.set_peer_counter(handle.peer_counter());

    let mut manager = InterfaceManager::new(8);
    let host = manager.new_channel_with_role(8, IfaceRole::Multicast);
    let host_iface = host.address;
    let uncounted = manager.new_channel(8);

    assert_eq!(manager.peer_count(&host_iface), None);
    assert!(manager.set_peer_counter(&host_iface, state.peer_counter()));
    assert_eq!(manager.peer_count(&host_iface), Some(0));
    assert_eq!(manager.peer_count(&uncounted.address), None);

    let admitted_at = plan.startup_plan.initial_peering_wait;
    let admitted = plan
        .process_discovery_datagram(&mut state, discovery_datagram, admitted_at)
        .expect("authenticated discovery datagram")
        .expect("final init should admit discovery");
    assert_eq!(admitted.event, AutoDiscoveryEvent::Peer(crate::iface::auto::AutoPeerEvent::Added));
    assert_eq!(state.peer_count(), 1);
    assert_eq!(manager.peer_count(&host_iface), Some(1));
    assert_eq!(handle.peer_count(), 1);

    let child = manager
        .register_virtual_iface(host_iface, IfaceRole::VirtualUnicast)
        .expect("register a virtual child on the host");
    assert_eq!(manager.peer_count(&child), Some(1));

    let expired = state
        .expire_stale_peers(admitted_at + peering_timeout + core::time::Duration::from_secs(1));
    assert_eq!(expired.len(), 1);
    assert_eq!(state.peer_count(), 0);
    assert_eq!(manager.peer_count(&host_iface), Some(0));
    assert_eq!(manager.peer_count(&child), Some(0));
    assert_eq!(manager.peer_count(&uncounted.address), None);
}

#[cfg(unix)]
async fn wait_for_peer_count(handle: &AutoRuntimeStatusHandle, expected: u64) {
    let deadline = Instant::now() + core::time::Duration::from_secs(5);
    loop {
        let observed = handle.peer_count();
        if observed == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for peer_count() == {expected}: last read {observed}"
        );
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    }
}

/// A handle passed to the spawn reads the peer the runtime admits from a
/// real loopback discovery socket, without any other hook into the state.
#[cfg(unix)]
#[tokio::test]
async fn spawned_runtime_publishes_peer_count_through_status_handle() {
    let data_reservation = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .expect("reserve data loopback UDP port");
    let data_port = data_reservation.local_addr().expect("read data port").port();
    let discovery_port = reserve_loopback_udp_port_below_max();
    drop(data_reservation);
    let mut plan =
        build_startup_plan_from_candidates(&auto_iface(), Vec::new()).expect("startup plan");
    plan.startup_plan.initial_peering_wait = core::time::Duration::ZERO;
    plan.startup_plan.discovery_listeners.push(AutoDiscoveryListenerBinding {
        ifname: "lo".to_string(),
        link_local_address: "127.0.0.1".to_string(),
        unicast_bind_address: "127.0.0.1".to_string(),
        unicast_bind_port: discovery_port,
        multicast_group_address: "239.255.0.1".to_string(),
        multicast_bind_address: "239.255.0.1".to_string(),
        multicast_bind_port: discovery_port,
    });
    plan.startup_plan.data_listeners.push(AutoDataListenerBinding {
        ifname: "lo".to_string(),
        link_local_address: "127.0.0.1".to_string(),
        bind_address: "127.0.0.1".to_string(),
        bind_port: data_port,
    });

    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    assert_eq!(handle.peer_count(), 0);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(None, Some(handle.clone()))
        .await
        .expect("start loopback AutoInterface runtime");
    assert_eq!(handle.peer_count(), 0);

    let peer_socket = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind loopback peer socket");
    let token = crate::iface::auto::peering_token(plan.config.group_id.as_bytes(), "127.0.0.1");
    peer_socket
        .send_to(&token, (std::net::Ipv4Addr::LOCALHOST, discovery_port))
        .await
        .expect("send peering token to the unicast discovery socket");

    wait_for_peer_count(&handle, 1).await;
    assert_eq!(handle.peer_count(), 1);
    runtime.stop().await;
}
