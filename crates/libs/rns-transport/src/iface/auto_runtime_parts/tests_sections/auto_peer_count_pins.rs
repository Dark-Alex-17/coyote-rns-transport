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

fn two_peer_table() -> crate::iface::auto::AutoPeerTable {
    let mut peers = crate::iface::auto::AutoPeerTable::new(
        core::time::Duration::from_secs(22),
        core::time::Duration::from_millis(5_200),
    );
    peers.observe_peer("fe80::1", "eth0", core::time::Duration::from_secs(10));
    peers.observe_peer("fe80::2", "wlan0", core::time::Duration::from_secs(10));
    assert_eq!(peers.peer_counter().load(std::sync::atomic::Ordering::Acquire), 2);
    peers
}

/// Removing an adopted device's peers lowers the counter with the table,
/// the same as expiry does.
#[test]
fn peer_table_counter_drops_when_a_device_is_removed() {
    let mut peers = two_peer_table();
    let counter = peers.peer_counter();

    let removed = peers.remove_by_ifname("eth0");

    assert_eq!(removed.len(), 1);
    assert_eq!(peers.len(), 1);
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 1);
}

/// A counter installed on a populated table reads the table's length at
/// once, whatever it held before; the counter it replaces stops tracking.
#[test]
fn peer_table_publishes_its_length_into_a_newly_installed_counter() {
    let mut peers = two_peer_table();
    let replaced = peers.peer_counter();
    let installed = Arc::new(std::sync::atomic::AtomicU64::new(99));

    peers.set_peer_counter(Arc::clone(&installed));

    assert_eq!(installed.load(std::sync::atomic::Ordering::Acquire), 2);
    peers.observe_peer("fe80::3", "eth0", core::time::Duration::from_secs(11));
    assert_eq!(installed.load(std::sync::atomic::Ordering::Acquire), 3);
    assert_eq!(replaced.load(std::sync::atomic::Ordering::Acquire), 2);
}

/// The handle's JSON carries the counter as `peers`: 0 for a fresh handle,
/// and whatever the shared counter holds afterwards.
#[test]
fn status_handle_json_carries_the_peer_count() {
    let plan = loopback_software_auto_plan(core::time::Duration::from_millis(50));
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    assert_eq!(handle.to_json()["peers"].as_u64(), Some(0));

    handle.peer_counter().store(3, std::sync::atomic::Ordering::Release);

    assert_eq!(handle.peer_count(), 3);
    assert_eq!(handle.to_json()["peers"].as_u64(), Some(3));
    assert_eq!(handle.clone().to_json()["peers"].as_u64(), Some(3));
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

/// A plan with one loopback discovery listener and one loopback data
/// listener on fresh ports, no adopted devices and no initial peering wait,
/// so a datagram from 127.0.0.1 is admitted as a remote peer at once.
/// Returns the plan and the unicast discovery port to send to.
#[cfg(unix)]
fn loopback_runtime_plan() -> (AutoRuntimePlan, u16) {
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
    (plan, discovery_port)
}

#[cfg(unix)]
async fn send_loopback_peering_token(plan: &AutoRuntimePlan, discovery_port: u16) {
    let peer_socket = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind loopback peer socket");
    let token = crate::iface::auto::peering_token(plan.config.group_id.as_bytes(), "127.0.0.1");
    peer_socket
        .send_to(&token, (std::net::Ipv4Addr::LOCALHOST, discovery_port))
        .await
        .expect("send peering token to the unicast discovery socket");
}

/// A handle passed to the spawn reads the peer the runtime admits from a
/// real loopback discovery socket, without any other hook into the state.
#[cfg(unix)]
#[tokio::test]
async fn spawned_runtime_publishes_peer_count_through_status_handle() {
    let (plan, discovery_port) = loopback_runtime_plan();

    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    assert_eq!(handle.peer_count(), 0);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(None, Some(handle.clone()))
        .await
        .expect("start loopback AutoInterface runtime");
    assert_eq!(handle.peer_count(), 0);

    send_loopback_peering_token(&plan, discovery_port).await;

    wait_for_peer_count(&handle, 1).await;
    assert_eq!(handle.peer_count(), 1);
    assert_eq!(handle.to_json()["peers"].as_u64(), Some(1));
    runtime.stop().await;
}

/// Spawning with a transport runtime installs the handle's counter on the
/// bridge's host channel, the one route by which
/// `Transport::interface_peer_count` answers `Some` in the daemon: the host
/// reads `Some(0)` right after the spawn, `Some(1)` with the handle once a
/// peer is admitted, the same through the virtual child the bridge
/// registers for that peer, and still `Some(1)` after stop, which leaves the
/// manager's host record alone: the runtime never binds the record's stop
/// token, so a cleanup pass keeps the record too. The record goes when a
/// dispatch observes the stopped runtime's closed tx queue; the read is
/// `None` from then on while the handle keeps the last value.
#[cfg(unix)]
#[tokio::test]
async fn spawned_runtime_installs_peer_count_on_the_bridge_host_channel() {
    let (plan, discovery_port) = loopback_runtime_plan();
    let manager = Arc::new(tokio::sync::Mutex::new(InterfaceManager::new(8)));
    let channel = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast);
    let host = channel.address;
    let transport_runtime =
        AutoInterfaceTransportRuntime::from_channel(channel, Arc::clone(&manager));
    assert_eq!(manager.lock().await.peer_count(&host), None);

    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(
            Some(transport_runtime),
            Some(handle.clone()),
        )
        .await
        .expect("start loopback AutoInterface runtime with a transport bridge");
    assert_eq!(manager.lock().await.peer_count(&host), Some(0));
    assert_eq!(handle.peer_count(), 0);

    send_loopback_peering_token(&plan, discovery_port).await;

    wait_for_peer_count(&handle, 1).await;
    assert_eq!(manager.lock().await.peer_count(&host), Some(1));

    let deadline = Instant::now() + core::time::Duration::from_secs(5);
    let child = loop {
        let registered = manager.lock().await.interface_hashes();
        if let Some(child) = registered.into_iter().find(|address| *address != host) {
            break child;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the bridge to register the admitted peer's child"
        );
        tokio::time::sleep(core::time::Duration::from_millis(20)).await;
    };
    assert_eq!(manager.lock().await.peer_count(&child), Some(1));

    runtime.stop().await;
    assert!(
        manager.lock().await.interface_hashes().contains(&host),
        "stop leaves the host record in the manager"
    );
    assert_eq!(
        manager.lock().await.peer_count(&host),
        Some(1),
        "the host's counter holds its last value: stop does not zero it"
    );
    assert_eq!(
        handle.peer_count(),
        1,
        "the handle reads the same counter, which stop does not zero"
    );

    manager.lock().await.cleanup();
    assert!(
        manager.lock().await.interface_hashes().contains(&host),
        "a cleanup pass does not drop a host whose stop token was never cancelled"
    );
    assert_eq!(
        manager.lock().await.peer_count(&host),
        Some(1),
        "after a cleanup pass the surviving host still reads its last value"
    );

    manager
        .lock()
        .await
        .send(TxMessage {
            tx_type: TxMessageType::Broadcast(None),
            packet: test_packet(b"after-stop", 0x66),
        })
        .await;
    assert_eq!(
        manager.lock().await.peer_count(&host),
        None,
        "a dispatch that observes the closed tx queue drops the host record"
    );
    assert_eq!(handle.peer_count(), 1, "the handle keeps the last value after the prune");
}
