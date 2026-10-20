// Coverage for the shared-instance client API: two client transports attached to one TCP
// shared instance learn each other's announces at one hop and open links both ways, a
// second attach is refused until the first is detached, and a unix attach registers its
// client interface before anything listens on the socket. Helpers come from the sibling
// `tests_parts` files included into this module.

#[cfg(unix)]
use crate::iface::local::LocalUnixEndpoint;
use std::net::SocketAddr;

/// The shared instance: a transport node with a `TcpServer` bound on 127.0.0.1 that
/// relays between the clients attached to it.
struct SharedInstanceServer {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
}

impl SharedInstanceServer {
    fn endpoint(&self) -> SharedInstanceEndpoint {
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], self.port)))
    }

    #[deny(clippy::await_holding_invalid_type)]
    async fn await_accepted_connections(&self, count: u64) {
        wait_until(
            "shared instance to accept",
            || self.status.to_json()["accepted_connections"].as_u64() == Some(count),
            || self.status.to_json().to_string(),
        )
        .await;
    }
}

/// A client of the shared instance: broadcast and transport both off. The announce
/// receiver is subscribed before the attach so the first relayed announce cannot be missed.
struct SharedInstanceClient {
    transport: Transport,
    announces: broadcast::Receiver<AnnounceEvent>,
    iface: AddressHash,
}

#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_server() -> SharedInstanceServer {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("shared-instance", &identity, false);
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager())
        .with_client_mtu(TcpClient::DEFAULT_MTU);
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until(
        "shared instance to listen",
        || status.to_json()["listener_state"].as_str() == Some("listening"),
        || status.to_json().to_string(),
    )
    .await;
    SharedInstanceServer { transport, port, status }
}

#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_client(name: &str, server: &SharedInstanceServer) -> SharedInstanceClient {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("attach to the shared instance");
    SharedInstanceClient { transport, announces, iface }
}

/// Consumes announces until the one for `hash` arrives and returns the learned description
/// with the hop count the client saw.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_learn_announce(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> (DestinationDesc, u8) {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let desc = event.destination.lock().await.desc;
                    if desc.address_hash == *hash {
                        return (desc, event.hops);
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .expect("timed out waiting for the announce")
}

#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_assert_attached(transport: &Transport, iface: AddressHash) {
    assert_eq!(transport.shared_instance(), Some(iface), "the slot names the attached iface");
    assert!(transport.interface_hashes().await.contains(&iface), "the iface is registered");
    assert!(transport.is_shared_instance(&iface).await, "the iface is the shared instance");
    assert!(transport.connected_to_shared_instance().await, "the client posture is set");
    let is_local_client = {
        let manager = transport.iface_manager();
        let manager = manager.lock().await;
        manager.is_local_client_interface(&iface)
    };
    assert!(!is_local_client, "the attached iface has no shared-instance parent");
}

#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_assert_released(transport: &Transport, iface: AddressHash) {
    assert_eq!(transport.shared_instance(), None, "the slot is cleared");
    assert!(!transport.interface_hashes().await.contains(&iface), "the iface is gone");
    assert!(!transport.is_shared_instance(&iface).await, "the iface is no shared instance");
    assert!(!transport.connected_to_shared_instance().await, "the client posture is cleared");
    let is_local_client = {
        let manager = transport.iface_manager();
        let manager = manager.lock().await;
        manager.is_local_client_interface(&iface)
    };
    assert!(!is_local_client, "a released iface is not a local client");
}

/// Both clients send at hops 0. The shared instance's receive increment makes 1 and it
/// relays at 1; the receiving client's increment makes 2 and its shared-instance boundary
/// takes one back off, so each client learns the other's destination at one hop.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_clients_exchange_announces_and_open_links_both_ways() {
    let server = shared_instance_server().await;
    let mut a = shared_instance_client("shared-instance-client-a", &server).await;
    let mut b = shared_instance_client("shared-instance-client-b", &server).await;
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&a.transport, a.iface).await;
    shared_instance_assert_attached(&b.transport, b.iface).await;
    let mut a_in_events = a.transport.in_link_events();
    let mut b_in_events = b.transport.in_link_events();

    let (_a_destination, a_desc) =
        register_in_destination(&a.transport, "shared-instance", "a").await;
    let (_b_destination, b_desc) =
        register_in_destination(&b.transport, "shared-instance", "b").await;
    let (a_learned, hops) =
        shared_instance_learn_announce(&mut b.announces, &a_desc.address_hash).await;
    assert_eq!(hops, 1, "b learns a's destination at one hop");
    let (b_learned, hops) =
        shared_instance_learn_announce(&mut a.announces, &b_desc.address_hash).await;
    assert_eq!(hops, 1, "a learns b's destination at one hop");

    let mut b_out_events = b.transport.out_link_events();
    let (_link_to_a, link_id) = open_link(&b.transport, a_learned).await;
    let activated =
        await_event(&mut a_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    let report =
        b.transport.send_to_out_links_with_report(&a_desc.address_hash, b"ping from b").await;
    assert_sent_on_one_link(report, "outbound");
    let inbound = await_event(&mut a_in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(inbound), b"ping from b");

    let report =
        a.transport.send_to_in_links_with_report(&a_desc.address_hash, b"pong from a").await;
    assert_sent_on_one_link(report, "inbound");
    let outbound = await_event(&mut b_out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(outbound), b"pong from a");

    let (_link_to_b, link_id) = open_link(&a.transport, b_learned).await;
    let activated =
        await_event(&mut b_in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "the reverse link shares one id too");

    let detached = timeout(STOP_TIMEOUT, a.transport.detach_shared_instance())
        .await
        .expect("detach completes");
    assert!(detached, "detach reports the attached shared instance");
    shared_instance_assert_released(&a.transport, a.iface).await;

    timeout(STOP_TIMEOUT, b.transport.stop()).await.expect("client stop completes");
    shared_instance_assert_released(&b.transport, b.iface).await;

    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_second_attach_is_refused_until_detached() {
    let server = shared_instance_server().await;
    let client = shared_instance_client("shared-instance-client", &server).await;
    server.await_accepted_connections(1).await;
    let registered = client.transport.interface_hashes().await.len();

    let refused = client
        .transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await;
    assert!(
        matches!(refused, Err(RnsError::InvalidArgument)),
        "a second attach is refused while one is held: {refused:?}"
    );
    assert_eq!(client.transport.shared_instance(), Some(client.iface), "the first attach is kept");
    assert_eq!(
        client.transport.interface_hashes().await.len(),
        registered,
        "a refused attach spawns no interface"
    );

    let detached = timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
        .await
        .expect("detach completes");
    assert!(detached, "detach reports the attached shared instance");
    shared_instance_assert_released(&client.transport, client.iface).await;

    let reattached = client
        .transport
        .attach_shared_instance(server.endpoint(), SharedInstanceOptions::default())
        .await
        .expect("a fresh attach succeeds after detach");
    assert_ne!(reattached, client.iface, "the fresh attach spawns a new interface");
    server.await_accepted_connections(2).await;
    shared_instance_assert_attached(&client.transport, reattached).await;

    let detached = timeout(STOP_TIMEOUT, client.transport.detach_shared_instance())
        .await
        .expect("second detach completes");
    assert!(detached, "detach reports the fresh attach");
    assert!(
        !client.transport.detach_shared_instance().await,
        "detach with nothing attached reports false"
    );
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The unix client worker keeps retrying an absent socket, so the attach succeeds and the
/// interface stays registered until detach cancels it.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_unix_attach_registers_a_local_unix_client() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("shared-instance-unix", &identity, false));
    let socket =
        std::env::temp_dir().join(format!("rns-shared-{}", std::process::id())).join("absent.sock");

    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket)),
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(50)),
        )
        .await
        .expect("attach succeeds before the socket exists");
    shared_instance_assert_attached(&transport, iface).await;

    let detached = timeout(STOP_TIMEOUT, transport.detach_shared_instance())
        .await
        .expect("detach completes while the worker is retrying");
    assert!(detached, "detach reports the attached shared instance");
    shared_instance_assert_released(&transport, iface).await;
}
