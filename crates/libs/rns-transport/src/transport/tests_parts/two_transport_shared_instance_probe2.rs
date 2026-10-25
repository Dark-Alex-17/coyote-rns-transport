// Second usage-pattern probe for the shared-instance client API: the unix flavour carried
// end to end (two clients on one `LocalUnixServer` learn each other at one hop and open links
// both ways), the TCP client retrying at the caller's shortened
// `reconnect_wait`, `stop()` forgetting the slot inside its budget while the handler is
// wedged, and the slot surviving a `stop_interface` on the attached interface until
// `detach_shared_instance` clears it. Helpers come from the sibling `tests_parts` files
// included into this module.

/// How long the TCP probe gives the client to reconnect. Shorter than the `TcpClient`'s 5 s
/// default wait by a wide margin, so only a retry at the caller's 50 ms `reconnect_wait`
/// can make the accept land inside it.
const SHARED_INSTANCE_TCP_EARLY_WINDOW: Duration = Duration::from_secs(2);

/// The unix shared instance: a transport node with a `LocalUnixServer` bound on a socket
/// under `dir`. The server exposes no listener status, so the socket's appearance on disk is
/// the listening signal.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_unix_owner(dir: &std::path::Path) -> (Transport, LocalUnixEndpoint) {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("shared-instance-unix-owner", &identity, false);
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let socket = dir.join("shared.sock");
    let endpoint = LocalUnixEndpoint::filesystem(socket.clone());
    let server = crate::iface::local::LocalUnixServer::new_endpoint(
        endpoint.clone(),
        transport.iface_manager(),
    );
    transport
        .iface_manager()
        .lock()
        .await
        .spawn(server, crate::iface::local::LocalUnixServer::spawn);
    wait_until(
        "the unix shared instance to bind its socket",
        || socket.exists(),
        || socket.display().to_string(),
    )
    .await;
    (transport, endpoint)
}

/// Polls `transport.interface_hashes()` until it holds `count` entries: the listener plus one
/// accepted child per connected client.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_await_interface_count(transport: &Transport, count: usize) {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    loop {
        let registered = transport.interface_hashes().await.len();
        if registered == count {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {count} interfaces on the shared instance, saw {registered}"
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// A client attached over `endpoint` with default options, its announce receiver subscribed
/// before the attach.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_attach_client(
    name: &str,
    endpoint: &SharedInstanceEndpoint,
) -> SharedInstanceClient {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(endpoint.clone(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    SharedInstanceClient { transport, announces, iface }
}

/// The unix flavour is the same posture as TCP: each client sends at hops 0, the shared
/// instance relays at 1, and the receiving client's shared-instance boundary takes the
/// relay hop back off, so both learn the other at one hop and a link opens each way.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_unix_clients_exchange_announces_and_open_links_both_ways() {
    let temp = tempfile::tempdir().expect("tempdir");
    let (owner, unix_endpoint) = shared_instance_unix_owner(temp.path()).await;
    let endpoint = SharedInstanceEndpoint::Unix(unix_endpoint);
    assert_eq!(endpoint.clone(), endpoint, "the endpoint is a plain value a caller can keep");
    let owner_ifaces_alone = owner.interface_hashes().await.len();

    let mut a = shared_instance_attach_client("shared-instance-unix-a", &endpoint).await;
    let mut b = shared_instance_attach_client("shared-instance-unix-b", &endpoint).await;
    shared_instance_assert_attached(&a.transport, a.iface).await;
    shared_instance_assert_attached(&b.transport, b.iface).await;
    shared_instance_await_interface_count(&owner, owner_ifaces_alone + 2).await;

    let (_a_destination, a_desc) =
        register_in_destination(&a.transport, "shared-instance-unix", "a").await;
    let (a_learned, hops) =
        shared_instance_learn_announce(&mut b.announces, &a_desc.address_hash).await;
    assert_eq!(hops, 1, "b learns a's destination at one hop over the unix shared instance");
    let (_b_destination, b_desc) =
        register_in_destination(&b.transport, "shared-instance-unix", "b").await;
    let (b_learned, hops) =
        shared_instance_learn_announce(&mut a.announces, &b_desc.address_hash).await;
    assert_eq!(hops, 1, "a learns b's destination at one hop over the unix shared instance");

    let mut b_in_events = b.transport.in_link_events();
    let (_link_to_b, link_id) = open_link(&a.transport, b_learned).await;
    let activated =
        await_event(&mut b_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "a link opens a -> b over the unix shared instance");

    let mut a_in_events = a.transport.in_link_events();
    let (_link_to_a, link_id) = open_link(&b.transport, a_learned).await;
    let activated =
        await_event(&mut a_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "a link opens b -> a over the unix shared instance");

    let detached = timeout(STOP_TIMEOUT, a.transport.detach_shared_instance())
        .await
        .expect("detach completes");
    assert!(detached, "detach reports the attached unix shared instance");
    shared_instance_assert_released(&a.transport, a.iface).await;
    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("client stop completes");
    shared_instance_assert_released(&b.transport, b.iface).await;
    timeout(STOP_TIMEOUT, owner.stop()).await.expect("unix shared instance stop completes");
}

/// A TCP client given a 50 ms `reconnect_wait` retries at that wait rather than the
/// `TcpClient`'s 5 s default, so a shared instance that comes up right after the first
/// refused connect is reached inside two seconds.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_client_honours_a_short_reconnect_wait() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport =
        Transport::new(TransportConfig::new("shared-instance-short-wait", &identity, false));
    let port = free_loopback_port().await;
    let endpoint = SharedInstanceEndpoint::Tcp(std::net::SocketAddr::from(([127, 0, 0, 1], port)));

    let iface = transport
        .attach_shared_instance(
            endpoint,
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(50)),
        )
        .await
        .expect("attach succeeds before anything listens");
    shared_instance_assert_attached(&transport, iface).await;
    // Let the client's first connect be refused by the closed port before the shared
    // instance binds it; a refused loopback connect returns at once.
    tokio::time::sleep(Duration::from_millis(300)).await;

    let server_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut server_config =
        TransportConfig::new("shared-instance-short-wait-owner", &server_identity, false);
    server_config.set_transport_enabled(true);
    let server = Transport::new(server_config);
    let listener = TcpServer::new(format!("127.0.0.1:{port}"), server.iface_manager())
        .with_client_mtu(TcpClient::DEFAULT_MTU);
    let status = listener.runtime_status_handle();
    server.iface_manager().lock().await.spawn(listener, TcpServer::spawn);

    let accepted = timeout(SHARED_INSTANCE_TCP_EARLY_WINDOW, async {
        while status.to_json()["accepted_connections"].as_u64() != Some(1) {
            tokio::time::sleep(LOOPBACK_POLL).await;
        }
    })
    .await;
    assert!(
        accepted.is_ok(),
        "the TCP client must retry at the 50 ms reconnect_wait, not its 5 s default: {}",
        status.to_json()
    );
    shared_instance_assert_attached(&transport, iface).await;

    let detached = timeout(STOP_TIMEOUT, transport.detach_shared_instance())
        .await
        .expect("detach completes while the client is connected");
    assert!(detached, "detach reports the attached shared instance");
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, server.stop()).await.expect("shared instance stop completes");
}

/// `stop()` with the handler wedged for good: the interface is still detached with the
/// rest, the slot is still forgotten, and stop returns once its budget is spent. The guard
/// is leaked on purpose and never held across an await; the posture flag lives behind the
/// wedged handler, so only the slot and the interface tables are read afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_stop_with_the_handler_wedged_still_forgets_the_slot_within_budget() {
    let server = shared_instance_server().await;
    let client = shared_instance_client("shared-instance-wedged", &server).await;
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&client.transport, client.iface).await;

    let handler = client.transport.get_handler();
    let wedge = handler.lock().await;
    std::mem::forget(wedge);

    timeout(STOP_TIMEOUT + Duration::from_secs(3), client.transport.stop())
        .await
        .expect("stop returns once its budget is spent even with the handler wedged");
    assert_eq!(client.transport.shared_instance(), None, "the slot is forgotten");
    assert!(
        !client.transport.interface_hashes().await.contains(&client.iface),
        "the interface is detached with the rest"
    );
    assert!(
        !client.transport.is_shared_instance(&client.iface).await,
        "the detached interface is no shared instance"
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// `stop_interface` leaves the slot set, as its rustdoc says; the shared body lives beside
/// the `remove_interface` twin.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_removed_through_stop_interface_keeps_the_slot_until_detach() {
    shared_instance_removed_keeps_the_slot_until_detach(
        "shared-instance-stop-iface",
        SharedInstanceRemoval::StopInterface,
    )
    .await;
}
