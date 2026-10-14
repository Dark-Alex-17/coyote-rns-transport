use crate::iface::tcp_client::TcpClient;
use crate::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use tokio::sync::broadcast::error::RecvError;

const LOOPBACK_TIMEOUT: Duration = Duration::from_secs(15);
const LOOPBACK_POLL: Duration = Duration::from_millis(20);

/// Listening side of the loopback pair: a transport with a `TcpServer` bound on 127.0.0.1.
struct LoopbackListener {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
}

impl LoopbackListener {
    /// Waits until the server has registered `count` accepted peers. A connector's own
    /// `connected` state only says the stream is up; the accepted child interface that
    /// carries the listener's traffic is registered a task wake later.
    #[deny(clippy::await_holding_invalid_type)]
    async fn await_accepted_connections(&self, count: u64) {
        wait_until(
            "listener to accept",
            || self.status.to_json()["accepted_connections"].as_u64() == Some(count),
            || self.status.to_json().to_string(),
        )
        .await;
    }
}

/// Connecting side: a transport with a `TcpClient` attached to the listener's port. The
/// announce receiver is subscribed before the TCP stream is up so the listener's first
/// announce cannot be missed.
struct LoopbackConnector {
    transport: Transport,
    announces: broadcast::Receiver<AnnounceEvent>,
}

/// `TcpServer` reports the configured bind string in its status, not the bound port, so a
/// `:0` bind would not reveal the port. Probe-bind to find a free one and release it.
#[deny(clippy::await_holding_invalid_type)]
async fn free_loopback_port() -> u16 {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind probe listener");
    let port = probe.local_addr().expect("probe listener address").port();
    drop(probe);
    port
}

/// Polls `condition` until it holds; on timeout the panic carries `detail()` so a bind or
/// connect failure recorded in the interface status is visible instead of a bare timeout.
#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(
    what: &str,
    mut condition: impl FnMut() -> bool,
    detail: impl Fn() -> String,
) {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {}",
            detail()
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn listener(name: &str, client_mtu: usize) -> LoopbackListener {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager())
        .with_client_mtu(client_mtu);
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until(
        "listener to listen",
        || status.to_json()["listener_state"].as_str() == Some("listening"),
        || status.to_json().to_string(),
    )
    .await;
    LoopbackListener {
        transport,
        port,
        status,
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn connector(name: &str, port: u16, mtu: usize) -> LoopbackConnector {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}")).with_mtu(mtu);
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until(
        "connector to connect",
        || status.to_json()["stream_state"].as_str() == Some("connected"),
        || status.to_json().to_string(),
    )
    .await;
    LoopbackConnector { transport, announces }
}

/// Registers an IN single destination on `transport` and announces it once.
#[deny(clippy::await_holding_invalid_type)]
async fn register_in_destination(
    transport: &Transport,
    app: &str,
    aspect: &str,
) -> (Arc<Mutex<SingleInputDestination>>, DestinationDesc) {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let destination = transport.add_destination(identity, DestinationName::new(app, aspect)).await;
    let desc = destination.lock().await.desc;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    transport.send_packet(packet).await;
    (destination, desc)
}

/// Consumes announces until the one for `hash` arrives and returns the learned description.
#[deny(clippy::await_holding_invalid_type)]
async fn learn_announce(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> DestinationDesc {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let desc = event.destination.lock().await.desc;
                    if desc.address_hash == *hash {
                        return desc;
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

/// Waits for the first link event matching `predicate`, skipping everything else.
#[deny(clippy::await_holding_invalid_type)]
async fn await_event(
    events: &mut broadcast::Receiver<LinkEventData>,
    mut predicate: impl FnMut(&LinkEventData) -> bool,
) -> LinkEventData {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if predicate(&event) => return event,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .expect("timed out waiting for the link event")
}

/// Requests a link to `desc` and waits for its `Activated` event on the initiator side.
#[deny(clippy::await_holding_invalid_type)]
async fn open_link(
    transport: &Transport,
    desc: DestinationDesc,
) -> (Arc<Mutex<Link>>, AddressHash) {
    let mut events = transport.out_link_events();
    let link = transport.link(desc).await;
    let link_id = *link.lock().await.id();
    await_event(&mut events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    (link, link_id)
}

fn data_bytes(event: LinkEventData) -> Vec<u8> {
    match event.event {
        LinkEvent::Data(payload) => payload.as_slice().to_vec(),
        _ => panic!("expected a Data event for link {}", event.id),
    }
}

/// A fan-out miss would otherwise only surface as the receiver's timeout.
fn assert_sent_on_one_link(report: LinkSendReport, direction: &str) {
    assert_eq!(
        (report.matched_links, report.sent_links, report.failed_links),
        (1, 1, 0),
        "{direction} send should reach exactly one active link: {report:?}"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_exchanges_data_both_ways() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();

    let (_destination, desc) =
        register_in_destination(&listener.transport, "loopback", "smoke").await;
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;

    let mut out_events = connector.transport.out_link_events();
    let (_link, link_id) = open_link(&connector.transport, learned).await;
    let activated = await_event(&mut in_events, |event| {
        matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    let report = connector
        .transport
        .send_to_out_links_with_report(&desc.address_hash, b"ping from connector")
        .await;
    assert_sent_on_one_link(report, "outbound");
    let inbound = await_event(&mut in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(inbound), b"ping from connector");

    let report = listener
        .transport
        .send_to_in_links_with_report(&desc.address_hash, b"pong from listener")
        .await;
    assert_sent_on_one_link(report, "inbound");
    let outbound = await_event(&mut out_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::Data(_))
    })
    .await;
    assert_eq!(data_bytes(outbound), b"pong from listener");
}
