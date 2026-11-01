//! The transport-wide interface event stream used from outside the crate:
//! `Transport::interface_events` is one broadcast of every stream client's connection
//! state, each `InterfaceEvent` tagged with the interface it belongs to, so a consumer
//! following a shared instance and a plain `TcpClient` on the same transport tells them
//! apart by `iface()`. Against raw listeners the way each stream ends is the test's to
//! choose: an orderly close reads `Disconnected { PeerClosed }` then the redial's
//! `Connecting`, and a port nothing listens on reads `DialFailed { Refused }` once per
//! attempt. The shared-instance state stream and the per-client senders keep working
//! beside the broadcast, a receiver dropped at once costs the client nothing, a receiver
//! that falls behind reads `Lagged` and never slows the publisher, publishing with no
//! subscriber or no stream is a silent no-op, a client built from a pre-supplied stream
//! (the shape a `TcpServer` runs for each accepted peer) publishes nothing, and the unix
//! client publishes the same four kinds as TCP.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::iface::{InterfaceChannel, InterfaceContext, InterfaceManager};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    DialFailureKind, DisconnectReason, InterfaceEvent, InterfaceEventKind, SharedInstanceEndpoint,
    SharedInstanceEvent, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// How long a stream is watched to show that nothing arrived on it.
const QUIET: Duration = Duration::from_millis(300);

fn client_transport(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}

#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn free_loopback_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

/// The next event on the broadcast. A `Lagged` is skipped, the stream closing is a
/// failure: the transport holds the sender for as long as it lives.
#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
) -> InterfaceEvent {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) => return event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
}

/// The broadcast read per interface. Two interfaces on one transport interleave their
/// events arbitrarily, so an event read while waiting on one interface is held for the
/// other rather than dropped; within one interface the order is the client's.
struct TaggedEvents {
    events: broadcast::Receiver<InterfaceEvent>,
    held: Vec<InterfaceEvent>,
}

impl TaggedEvents {
    fn new(events: broadcast::Receiver<InterfaceEvent>) -> Self {
        Self { events, held: Vec::new() }
    }

    /// The next event tagged with `iface`.
    #[deny(clippy::await_holding_invalid_type)]
    async fn next_for(&mut self, what: &str, iface: AddressHash) -> InterfaceEventKind {
        self.await_for(what, iface, |_| true).await
    }

    /// The first event tagged with `iface` whose kind satisfies `accept`; earlier events
    /// of that interface are consumed and skipped.
    #[deny(clippy::await_holding_invalid_type)]
    async fn await_for(
        &mut self,
        what: &str,
        iface: AddressHash,
        accept: impl Fn(InterfaceEventKind) -> bool,
    ) -> InterfaceEventKind {
        while let Some(at) = self.held.iter().position(|event| event.iface() == iface) {
            let kind = self.held.remove(at).kind();
            if accept(kind) {
                return kind;
            }
        }
        timeout(WAIT, async {
            loop {
                let event = next_event(what, &mut self.events).await;
                if event.iface() != iface {
                    self.held.push(event);
                } else if accept(event.kind()) {
                    return event.kind();
                }
            }
        })
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(WAIT, listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}

/// Closes `stream` in an orderly way: the write side is shut down and the stream read to
/// the client's EOF before it is dropped. An attached transport writes its tunnel packet
/// on each connect, and a TCP socket dropped with unread data in its receive buffer sends
/// a reset rather than a FIN.
#[deny(clippy::await_holding_invalid_type)]
async fn close_orderly(mut stream: impl AsyncRead + AsyncWrite + Unpin) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(WAIT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the client's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the client to close its side");
}

/// Attaches `transport` to the raw listener at `endpoint` with `options` plus a
/// 16-slot state stream and the short reconnect wait, and reads the attach's
/// `Connecting`; returns the stream's receiver.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(
    transport: &Transport,
    endpoint: SocketAddr,
    options: SharedInstanceOptions,
) -> mpsc::Receiver<SharedInstanceEvent> {
    let (events_tx, mut events) = mpsc::channel(16);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            options.with_reconnect_wait(RECONNECT_WAIT).with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(
        next_state("the attach's Connecting", &mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    events
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_state(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}

/// Serves one connection on `listener` and reads `Connected` for it on the state stream.
#[deny(clippy::await_holding_invalid_type)]
async fn serve_connected(
    listener: &TcpListener,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) -> TcpStream {
    let stream = accept_within(listener, what).await;
    assert_eq!(next_state(what, events).await, Some(SharedInstanceEvent::Connected), "{what}");
    stream
}

/// Spawns a plain `TcpClient` to `endpoint` through `transport`'s interface manager with
/// the short reconnect wait and returns its interface address.
#[deny(clippy::await_holding_invalid_type)]
async fn spawn_plain_client(transport: &Transport, endpoint: SocketAddr) -> AddressHash {
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    transport.iface_manager().lock().await.spawn(client, TcpClient::spawn)
}

/// One transport, one stream: the shared instance and a plain `TcpClient` each read
/// `Connecting` then `Connected` under their own address, `Disconnected { PeerClosed }`
/// then the redial's `Connecting` after an orderly close, and `DialFailed { Refused }`
/// once their listener is gone. The two interfaces interleave arbitrarily; the order
/// within each is fixed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_stream_carries_a_plain_tcp_client_and_the_shared_instance_tagged_by_iface() {
    let transport = client_transport("interface-events-two-ifaces");
    let mut events = TaggedEvents::new(transport.interface_events());
    let shared_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind shared listener");
    let shared_endpoint = shared_listener.local_addr().expect("shared listener address");
    let plain_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind plain listener");
    let plain_endpoint = plain_listener.local_addr().expect("plain listener address");

    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(shared_endpoint),
            SharedInstanceOptions::default().with_reconnect_wait(RECONNECT_WAIT),
        )
        .await
        .expect("the attach is accepted");
    let shared = transport.shared_instance().expect("the slot names the attached interface");
    let plain = spawn_plain_client(&transport, plain_endpoint).await;
    assert_ne!(shared, plain, "two interfaces, two addresses");

    let shared_stream = accept_within(&shared_listener, "the shared instance's connect").await;
    let plain_stream = accept_within(&plain_listener, "the plain client's connect").await;

    let mut seen: Vec<(AddressHash, InterfaceEventKind)> = Vec::new();
    let connected = |seen: &[(AddressHash, InterfaceEventKind)], iface: AddressHash| {
        seen.iter().any(|(tag, kind)| *tag == iface && *kind == InterfaceEventKind::Connected)
    };
    while !(connected(&seen, shared) && connected(&seen, plain)) {
        let event = next_event("a connect event", &mut events.events).await;
        assert!(
            event.iface() == shared || event.iface() == plain,
            "every event is tagged with one of the two interfaces: {event:?}"
        );
        seen.push((event.iface(), event.kind()));
    }
    for iface in [shared, plain] {
        let kinds: Vec<InterfaceEventKind> =
            seen.iter().filter(|(tag, _)| *tag == iface).map(|(_, kind)| *kind).collect();
        assert_eq!(
            kinds,
            [InterfaceEventKind::Connecting, InterfaceEventKind::Connected],
            "{iface}: one Connecting then one Connected"
        );
    }

    close_orderly(shared_stream).await;
    close_orderly(plain_stream).await;
    drop(shared_listener);
    drop(plain_listener);
    for (iface, what) in [(shared, "the shared instance"), (plain, "the plain client")] {
        assert_eq!(
            events.next_for(what, iface).await,
            InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed },
            "{what}: an orderly close reads PeerClosed"
        );
        assert_eq!(
            events.next_for(what, iface).await,
            InterfaceEventKind::Connecting,
            "{what}: the redial's Connecting follows the drop"
        );
        // A redial that raced the listener's drop may read a reset or even a short
        // `Connected`; the port is refused on every attempt once the listener is gone.
        let refused = InterfaceEventKind::DialFailed { reason: DialFailureKind::Refused };
        events.await_for(what, iface, |kind| kind == refused).await;
    }

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The senders that predate the broadcast keep their shape beside it. The state stream
/// an attach was given still reads `Connecting`, `Connected`, and the plain
/// `Disconnected` then `Connecting` for an orderly close; a plain client's
/// `with_connect_events` and `with_disconnect_reason_events` channels still fire. The
/// broadcast reads the same transitions under each interface's own address, the close
/// with its reason.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_shared_instance_state_stream_and_per_client_senders_are_unchanged() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let plain_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind plain listener");
    let plain_endpoint = plain_listener.local_addr().expect("plain listener address");
    let transport = client_transport("interface-events-beside-state-stream");
    let mut events = TaggedEvents::new(transport.interface_events());
    let mut state = attach(&transport, endpoint, SharedInstanceOptions::default()).await;
    let iface = transport.shared_instance().expect("the slot names the attached interface");
    let (connect_tx, mut connects) = mpsc::channel(4);
    let (reason_tx, mut reasons) = mpsc::channel(4);
    let plain = TcpClient::new(plain_endpoint.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_reason_events(reason_tx);
    let plain = transport.iface_manager().lock().await.spawn(plain, TcpClient::spawn);

    let stream = serve_connected(&listener, &mut state, "first connection").await;
    let plain_stream = accept_within(&plain_listener, "the plain client's connect").await;
    assert_eq!(
        timeout(WAIT, connects.recv()).await.expect("the connect event arrives"),
        Some(plain),
        "the per-client connect sender still fires"
    );
    close_orderly(stream).await;
    close_orderly(plain_stream).await;
    assert_eq!(
        next_state("the orderly close", &mut state).await,
        Some(SharedInstanceEvent::Disconnected),
        "the default attach keeps reading the plain Disconnected"
    );
    assert_eq!(
        next_state("the redial", &mut state).await,
        Some(SharedInstanceEvent::Connecting),
        "`Connecting` follows the drop at once"
    );
    assert_eq!(
        timeout(WAIT, reasons.recv()).await.expect("the disconnect reason arrives"),
        Some((plain, DisconnectReason::PeerClosed)),
        "the per-client disconnect reason sender still fires"
    );

    for tagged in [iface, plain] {
        for expected in [
            InterfaceEventKind::Connecting,
            InterfaceEventKind::Connected,
            InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed },
            InterfaceEventKind::Connecting,
        ] {
            assert_eq!(events.next_for("the broadcast twin", tagged).await, expected, "{tagged}");
        }
    }

    drop(listener);
    drop(plain_listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A receiver dropped as soon as it was subscribed leaves the client untouched: it
/// connects, survives an orderly close and redials as if nobody had ever subscribed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_dropped_receiver_costs_the_client_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("interface-events-dropped-receiver");
    drop(transport.interface_events());

    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    let first = accept_within(&listener, "the first connect").await;
    wait_until("the client to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    close_orderly(first).await;
    let second = accept_within(&listener, "the redial").await;

    drop(second);
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// With no sockets at all: a capacity-one broadcast behind a bare manager reads
/// `Lagged(1)` for the event the receiver missed, then the latest event tagged with the
/// channel's address, then nothing; the publisher never waited on either send.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_receiver_that_falls_behind_reads_lagged_and_the_publisher_never_blocks() {
    let mut manager = InterfaceManager::new(8);
    let (tx, mut rx) = broadcast::channel::<InterfaceEvent>(1);
    manager.set_interface_events(tx);
    let channel = manager.new_channel(8);

    channel.publish_event(InterfaceEventKind::Connecting);
    channel.publish_event(InterfaceEventKind::Connected);

    assert_eq!(rx.try_recv(), Err(TryRecvError::Lagged(1)));
    let event = rx.try_recv().expect("the latest event is still held");
    assert_eq!(event.kind(), InterfaceEventKind::Connected);
    assert_eq!(event.iface(), *channel.address());
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

/// Publishing is a no-op when nobody listens: a manager whose only receiver is gone and
/// a channel built with `InterfaceChannel::new`, which has no stream at all, both take
/// the call without a panic. `InterfaceEvent` is `Copy`, and the type the transport
/// re-exports is the type the `iface::events` module defines.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn publishing_with_no_subscriber_or_no_stream_is_a_silent_no_op() {
    let mut manager = InterfaceManager::new(8);
    let (tx, rx) = broadcast::channel::<InterfaceEvent>(4);
    manager.set_interface_events(tx);
    let channel = manager.new_channel(8);
    drop(rx);
    channel.publish_event(InterfaceEventKind::Connecting);
    channel.publish_event(InterfaceEventKind::DialFailed { reason: DialFailureKind::Refused });

    let (rx_tx, _rx_rx) = InterfaceChannel::make_rx_channel(8);
    let (_tx_tx, tx_rx) = InterfaceChannel::make_tx_channel(8);
    let bare = InterfaceChannel::new(
        rx_tx,
        tx_rx,
        AddressHash::new_from_rand(OsRng),
        CancellationToken::new(),
    );
    bare.publish_event(InterfaceEventKind::Connected);
    bare.publish_event(InterfaceEventKind::Disconnected { reason: DisconnectReason::Reset });

    let (tx, mut rx) = broadcast::channel::<InterfaceEvent>(4);
    let mut manager = InterfaceManager::new(8);
    manager.set_interface_events(tx);
    manager.new_channel(8).publish_event(InterfaceEventKind::Connected);
    let event = rx.try_recv().expect("a subscribed receiver reads the event");
    let copied = event;
    assert_eq!(event, copied, "InterfaceEvent is Copy");
    fn same<T>(_: &T, _: &T) {}
    let module_type: &rns_transport::iface::events::InterfaceEvent = &event;
    let transport_type: &rns_transport::transport::InterfaceEvent = &event;
    same(module_type, transport_type);
}

/// Carrying the event sender did not cost `InterfaceChannel` or `InterfaceContext` the
/// `UnwindSafe` and `RefUnwindSafe` they had before it.
#[test]
fn the_channel_and_context_stay_unwind_safe() {
    fn assert_unwind_safe<T: std::panic::UnwindSafe + std::panic::RefUnwindSafe>() {}
    assert_unwind_safe::<InterfaceChannel>();
    assert_unwind_safe::<InterfaceContext<TcpClient>>();
}

/// The stream covers dialing clients only: the connecting transport reads `Connecting`
/// then `Connected` under its own client's address, while the listening transport, whose
/// `TcpServer` runs each accepted peer as a client over a pre-supplied stream, reads
/// nothing for the accept and nothing for the peer's orderly exit once the connector stops.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_tcp_server_side_transport_publishes_nothing_for_its_accepted_peers() {
    let listener = client_transport("interface-events-server-side");
    let mut server_events = listener.interface_events();
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), listener.iface_manager());
    let server_status = server.runtime_status_handle();
    listener.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        server_status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;

    let connector = client_transport("interface-events-connector");
    let mut client_events = connector.interface_events();
    let client = TcpClient::new(format!("127.0.0.1:{port}")).with_reconnect_wait(RECONNECT_WAIT);
    let client_status = client.runtime_status_handle();
    let client_iface = connector.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    wait_until("the connector to connect", || {
        client_status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    wait_until("the listener to accept", || {
        server_status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    for expected in [InterfaceEventKind::Connecting, InterfaceEventKind::Connected] {
        let event = next_event("the connector's event", &mut client_events).await;
        assert_eq!(event.iface(), client_iface, "tagged with the spawned client");
        assert_eq!(event.kind(), expected);
    }
    tokio::time::sleep(QUIET).await;
    assert_eq!(server_events.try_recv(), Err(TryRecvError::Empty), "the accept publishes nothing");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    tokio::time::sleep(QUIET).await;
    assert_eq!(
        server_events.try_recv(),
        Err(TryRecvError::Empty),
        "the accepted peer's exit publishes nothing"
    );
    timeout(WAIT, listener.stop()).await.expect("listener stop completes");
}

/// A `TcpClient` built over an accepted raw socket is the shape a `TcpServer` spawns for
/// each peer; spawned through a transport it runs to the peer's close and publishes
/// nothing on the way, neither for the stream it was handed nor for its end.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_client_built_from_a_stream_publishes_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("listener address");
    let peer = TcpStream::connect(addr).await.expect("the peer connects");
    let accepted = accept_within(&listener, "the accept").await;

    let transport = client_transport("interface-events-from-stream");
    let mut events = transport.interface_events();
    let client = TcpClient::new_from_stream(addr.to_string(), accepted);
    let status = client.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    wait_until("the client to take up its stream", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;

    close_orderly(peer).await;
    wait_until("the client to finish with its stream", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    tokio::time::sleep(QUIET).await;
    assert_eq!(events.try_recv(), Err(TryRecvError::Empty));
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The unix client publishes the same four kinds: `Connecting` then
/// `DialFailed { NotFound }` while nothing is bound at the path, `Connected` once a
/// listener accepts, and `Disconnected { PeerClosed }` after an orderly close.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_unix_client_publishes_the_same_four_kinds() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("events.sock");
    let transport = client_transport("interface-events-unix");
    let mut events = TaggedEvents::new(transport.interface_events());
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT);
    let iface = transport.iface_manager().lock().await.spawn(client, LocalUnixClient::spawn);

    assert_eq!(events.next_for("the first dial", iface).await, InterfaceEventKind::Connecting);
    assert_eq!(
        events.next_for("the first failure", iface).await,
        InterfaceEventKind::DialFailed { reason: DialFailureKind::NotFound },
        "a path nothing has bound reads NotFound"
    );

    let listener =
        tokio::net::UnixListener::bind(&socket).expect("bind the socket the client dials");
    let (stream, _) = timeout(WAIT, listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed");
    let connected = events
        .await_for("the connect", iface, |kind| {
            !matches!(kind, InterfaceEventKind::Connecting | InterfaceEventKind::DialFailed { .. })
        })
        .await;
    assert_eq!(connected, InterfaceEventKind::Connected);

    close_orderly(stream).await;
    assert_eq!(
        events.next_for("the orderly close", iface).await,
        InterfaceEventKind::Disconnected { reason: DisconnectReason::PeerClosed }
    );

    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
