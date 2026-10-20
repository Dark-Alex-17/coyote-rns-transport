// Usage-pattern probes for the shared-instance client API, derived from the documented
// contract before the implementation was read: the option default, detach and stop on a
// transport that never attached, two attaches racing for the single slot, an attach
// issued before anything listens at the TCP endpoint, and the relay actually going quiet
// after a detach and coming back at one hop after a re-attach. Helpers come from the
// sibling `tests_parts` files included into this module.

/// How long the probe waits to be sure a relayed announce does NOT arrive. Only a slow
/// arrival could turn this into a false pass, never a false failure.
const SHARED_INSTANCE_QUIET_WINDOW: Duration = Duration::from_secs(1);

/// `reconnect_wait` defaults to `LocalClientInterface.RECONNECT_WAIT` (8 s,
/// `LocalInterface.py:53-54`) and the builder replaces it; the options type is `Clone` and
/// `Debug` so a caller can keep a template.
#[test]
fn shared_instance_options_default_reconnect_wait_is_eight_seconds() {
    let options = SharedInstanceOptions::default();
    assert_eq!(options.reconnect_wait(), Duration::from_secs(8), "LocalInterface.py:54");
    let template = options.clone();
    let shortened = options.with_reconnect_wait(Duration::from_millis(250));
    assert_eq!(shortened.reconnect_wait(), Duration::from_millis(250));
    assert_eq!(template.reconnect_wait(), Duration::from_secs(8), "the builder consumes a copy");
    assert!(format!("{shortened:?}").contains("250ms"), "Debug shows the wait in force");
}

/// A transport that never attached reports no shared instance: detach is `false` and
/// leaves the client posture off, and after `stop()` detach is still `false`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_detach_without_an_attach_reports_false_before_and_after_stop() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("shared-instance-never", &identity, false));
    assert_eq!(transport.shared_instance(), None);
    assert!(!transport.connected_to_shared_instance().await);

    let detached =
        timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach completes");
    assert!(!detached, "nothing was attached");
    assert_eq!(transport.shared_instance(), None);
    assert!(!transport.connected_to_shared_instance().await, "detach does not flip the posture");

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    let detached = timeout(STOP_TIMEOUT, transport.detach_shared_instance())
        .await
        .expect("detach after stop completes");
    assert!(!detached, "a stopped transport holds no shared instance");
    assert_eq!(transport.shared_instance(), None);
}

/// Two attaches issued concurrently for the single slot: exactly one wins, the other is
/// refused with `InvalidArgument`, and only the winner's interface exists.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_concurrent_attaches_admit_exactly_one() {
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("shared-instance-race", &identity, false));
    let before = transport.interface_hashes().await.len();

    let (first, second) = tokio::join!(
        transport.attach_shared_instance(server.endpoint(), SharedInstanceOptions::default()),
        transport.attach_shared_instance(server.endpoint(), SharedInstanceOptions::default()),
    );
    let (winner, refused) = match (first, second) {
        (Ok(winner), refused @ Err(_)) | (refused @ Err(_), Ok(winner)) => (winner, refused),
        other => panic!("exactly one concurrent attach must win: {other:?}"),
    };
    assert!(
        matches!(refused, Err(RnsError::InvalidArgument)),
        "the loser is refused as a second attach: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), Some(winner));
    assert_eq!(
        transport.interface_hashes().await.len(),
        before + 1,
        "the refused attach spawned no interface"
    );
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, winner).await;

    let detached =
        timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach completes");
    assert!(detached);
    shared_instance_assert_released(&transport, winner).await;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// Attaching before the shared instance listens is the normal start-up order for a
/// client program: the attach succeeds, the interface is held while the TCP client
/// retries (every 5 s, per the README), the client connects once the shared instance
/// comes up and relays its announce, and detach releases it afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_before_the_server_listens_connects_once_it_does() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("shared-instance-early", &identity, false));
    let mut announces = transport.recv_announces().await;
    let port = free_loopback_port().await;
    let endpoint = SharedInstanceEndpoint::Tcp(std::net::SocketAddr::from(([127, 0, 0, 1], port)));

    let iface = timeout(
        STOP_TIMEOUT,
        transport.attach_shared_instance(endpoint, SharedInstanceOptions::default()),
    )
    .await
    .expect("attach returns without waiting for a connection")
    .expect("attach succeeds before anything listens");
    shared_instance_assert_attached(&transport, iface).await;

    let server_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut server_config = TransportConfig::new("shared-instance-late", &server_identity, false);
    server_config.set_transport_enabled(true);
    let server = Transport::new(server_config);
    let listener = TcpServer::new(format!("127.0.0.1:{port}"), server.iface_manager())
        .with_client_mtu(TcpClient::DEFAULT_MTU);
    let status = listener.runtime_status_handle();
    server.iface_manager().lock().await.spawn(listener, TcpServer::spawn);
    wait_until(
        "late shared instance to accept the retrying client",
        || status.to_json()["accepted_connections"].as_u64() == Some(1),
        || status.to_json().to_string(),
    )
    .await;
    let (_server_destination, server_desc) =
        register_in_destination(&server, "shared-instance-early", "late").await;
    shared_instance_learn_announce(&mut announces, &server_desc.address_hash).await;
    shared_instance_assert_attached(&transport, iface).await;

    let detached = timeout(STOP_TIMEOUT, transport.detach_shared_instance())
        .await
        .expect("detach completes while the client is retrying");
    assert!(detached);
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    timeout(STOP_TIMEOUT, server.stop()).await.expect("late shared instance stop completes");
}

/// Detach is behavioural, not just bookkeeping: once `a` detaches, the shared instance
/// drops its side of the connection and `a` no longer learns what `b` announces; after
/// `a` re-attaches, a fresh announce from `b` arrives at one hop again and a link opens.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_detach_silences_the_relay_and_reattach_restores_one_hop_paths() {
    let server = shared_instance_server().await;
    let mut a = shared_instance_client("shared-instance-cycle-a", &server).await;
    let b = shared_instance_client("shared-instance-cycle-b", &server).await;
    server.await_accepted_connections(2).await;
    let server_ifaces_with_both = server.transport.interface_hashes().await.len();

    let (_b_first, b_first_desc) =
        register_in_destination(&b.transport, "shared-instance-cycle", "before").await;
    let (_, hops) =
        shared_instance_learn_announce(&mut a.announces, &b_first_desc.address_hash).await;
    assert_eq!(hops, 1, "baseline: a learns b at one hop");

    let detached = timeout(STOP_TIMEOUT, a.transport.detach_shared_instance())
        .await
        .expect("detach completes");
    assert!(detached);
    shared_instance_assert_released(&a.transport, a.iface).await;
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        let server_ifaces = server.transport.interface_hashes().await.len();
        if server_ifaces == server_ifaces_with_both - 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the shared instance to drop a's accepted connection: {server_ifaces} ifaces, expected {}",
            server_ifaces_with_both - 1
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }

    let (_b_second, b_second_desc) =
        register_in_destination(&b.transport, "shared-instance-cycle", "while-detached").await;
    let quiet = timeout(
        SHARED_INSTANCE_QUIET_WINDOW,
        shared_instance_learn_announce(&mut a.announces, &b_second_desc.address_hash),
    )
    .await;
    assert!(
        quiet.is_err(),
        "a detached client must not learn b's announce, but did at hops {:?}",
        quiet.map(|(_, hops)| hops)
    );

    let reattached = a
        .transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("re-attach succeeds after detach");
    server.await_accepted_connections(3).await;
    shared_instance_assert_attached(&a.transport, reattached).await;

    let (_b_third, b_third_desc) =
        register_in_destination(&b.transport, "shared-instance-cycle", "after-reattach").await;
    let (b_learned, hops) =
        shared_instance_learn_announce(&mut a.announces, &b_third_desc.address_hash).await;
    assert_eq!(hops, 1, "after re-attach a learns b at one hop again");

    let mut b_in_events = b.transport.in_link_events();
    let (_link, link_id) = open_link(&a.transport, b_learned).await;
    let activated =
        await_event(&mut b_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "a link opens over the re-attached shared instance");

    timeout(STOP_TIMEOUT, a.transport.stop()).await.expect("a stop completes");
    shared_instance_assert_released(&a.transport, reattached).await;
    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("b stop completes");
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}
