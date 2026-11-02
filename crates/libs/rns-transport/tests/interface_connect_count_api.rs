//! The per-interface connect counter used from outside the crate:
//! `Transport::interface_connect_count` reads how many streams a stream client has
//! established since it was spawned. Against raw listeners it reads `Some(0)` for a
//! client that has not connected, `Some(1)` after its first `Connected`, `Some(2)` after
//! the server drops the stream and the client redials, and `None` for an address the
//! transport never minted. A subscriber of `interface_events()` that read a `Connected`
//! reads at least that count, because the increment precedes the publish. The shared
//! instance's interface, named by `shared_instance()`, counts the same way across a
//! server-side drop, as does the unix client. For a `TcpClient` the same count is read
//! through `TcpRuntimeStatusHandle::connected_streams()` and the `"connected_streams"`
//! key of `runtime_status_json()` / `to_json()`, while a channel built with
//! `InterfaceChannel::new` owns a count no manager record sees.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::hash::{AddressHash, Hash};
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::{TcpClient, TcpRuntimeStatusHandle};
use rns_transport::iface::{InterfaceChannel, InterfaceManager};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    InterfaceEvent, InterfaceEventKind, SharedInstanceEndpoint, SharedInstanceEvent,
    SharedInstanceOptions, Transport,
};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(5);
const POLL: Duration = Duration::from_millis(20);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);

#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}

/// The next event tagged with `iface` whose kind is `kind`; other events are skipped.
#[deny(clippy::await_holding_invalid_type)]
async fn await_kind(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
    kind: InterfaceEventKind,
) {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.iface() == iface && event.kind() == kind => return,
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
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

#[deny(clippy::await_holding_invalid_type)]
async fn next_state(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}

/// Attaches `transport` to the raw listener at `endpoint` with a 16-slot state stream
/// and the short reconnect wait, and reads the attach's `Connecting`.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(
    transport: &Transport,
    endpoint: SocketAddr,
) -> mpsc::Receiver<SharedInstanceEvent> {
    let (events_tx, mut events) = mpsc::channel(16);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(
        next_state("the attach's Connecting", &mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    events
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
/// the short reconnect wait; returns its interface address and its status handle.
#[deny(clippy::await_holding_invalid_type)]
async fn spawn_plain_client(
    transport: &Transport,
    endpoint: SocketAddr,
) -> (AddressHash, TcpRuntimeStatusHandle) {
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    (iface, status)
}

/// The three TCP reads of the count, which must agree with each other.
fn tcp_status_count(status: &TcpRuntimeStatusHandle) -> u64 {
    let json = status.to_json();
    let count = status.connected_streams();
    assert_eq!(
        json["connected_streams"].as_u64(),
        Some(count),
        "to_json carries the getter's count"
    );
    count
}

/// A plain `TcpClient` through a transport: `Some(0)` before the listener accepts, `None`
/// for an address the transport never minted, `Some(1)` by the time a subscriber that
/// predates the spawn reads the first `Connected`, and `Some(2)` once the listener drops
/// the stream and the client redials. The client's status handle reads the same count at
/// each step through its getter and its JSON.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_plain_tcp_client_counts_each_established_stream() {
    let port = free_loopback_port().await;
    let endpoint: SocketAddr = format!("127.0.0.1:{port}").parse().expect("endpoint");
    let transport = client_transport("connect-count-plain");
    let mut events = transport.interface_events();
    let (iface, status) = spawn_plain_client(&transport, endpoint).await;

    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));
    assert_eq!(tcp_status_count(&status), 0);
    let unknown = AddressHash::new_from_hash(&Hash::new_from_slice(&[0xAAu8; 32]));
    assert_eq!(transport.interface_connect_count(&unknown).await, None);

    let listener = TcpListener::bind(endpoint).await.expect("bind the port the client dials");
    let stream = accept_within(&listener, "the first connect").await;
    await_kind("the first Connected", &mut events, iface, InterfaceEventKind::Connected).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(1),
        "the increment precedes the Connected a subscriber reads"
    );
    assert_eq!(tcp_status_count(&status), 1);

    close_orderly(stream).await;
    let stream = accept_within(&listener, "the redial").await;
    await_kind("the second Connected", &mut events, iface, InterfaceEventKind::Connected).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(2));
    assert_eq!(tcp_status_count(&status), 2);

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The count read through the transport and the count read through the client's status
/// handle are one count: pre-spawn `runtime_status_json()` reads 0 beside the handle's
/// JSON, and after a connect the handle reads what the transport reads.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_tcp_status_reads_the_transport_count() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let client = TcpClient::new(endpoint.to_string()).with_reconnect_wait(RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    assert_eq!(client.runtime_status_json()["connected_streams"].as_u64(), Some(0));
    assert_eq!(client.runtime_status_json(), status.to_json(), "pre-spawn, the two JSONs agree");
    assert_eq!(status.connected_streams(), 0);

    let transport = client_transport("connect-count-status");
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    let stream = accept_within(&listener, "the connect").await;
    wait_until("the status to count the stream", || status.connected_streams() == 1).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));
    assert_eq!(status.to_json()["stream_state"].as_str(), Some("connected"));

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The shared instance's interface, named by `shared_instance()`, reads `Some(1)` once
/// the attach's `Connected` is served and `Some(2)` after the server drops the stream and
/// the next connection is served.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_shared_instance_counts_across_a_server_side_drop() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("connect-count-shared");
    let mut state = attach(&transport, endpoint).await;
    let iface = transport.shared_instance().expect("the slot names the attached interface");

    let stream = serve_connected(&listener, &mut state, "the first connect").await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));

    close_orderly(stream).await;
    assert_eq!(
        next_state("the drop", &mut state).await,
        Some(SharedInstanceEvent::Disconnected),
        "the orderly close reads Disconnected"
    );
    assert_eq!(
        next_state("the redial", &mut state).await,
        Some(SharedInstanceEvent::Connecting),
        "Connecting follows the drop at once"
    );
    let stream = serve_connected(&listener, &mut state, "the reconnect").await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(2));

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The unix client counts the same way: `Some(0)` while nothing is bound at the path,
/// `Some(1)` once a listener accepts, `Some(2)` after the listener closes the stream and
/// the client redials.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_unix_client_counts_each_established_stream() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("count.sock");
    let transport = client_transport("connect-count-unix");
    let mut events = transport.interface_events();
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT);
    let iface = transport.iface_manager().lock().await.spawn(client, LocalUnixClient::spawn);

    await_kind("the first dial", &mut events, iface, InterfaceEventKind::Connecting).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));

    let listener =
        tokio::net::UnixListener::bind(&socket).expect("bind the socket the client dials");
    let (stream, _) = timeout(WAIT, listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed");
    await_kind("the first Connected", &mut events, iface, InterfaceEventKind::Connected).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));

    close_orderly(stream).await;
    let (stream, _) = timeout(WAIT, listener.accept())
        .await
        .expect("unix reconnection timed out")
        .expect("unix reconnection failed");
    await_kind("the second Connected", &mut events, iface, InterfaceEventKind::Connected).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(2));

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A manager reads `Some(0)` for a channel it minted and `None` for a channel built with
/// `InterfaceChannel::new`, whose count no manager record sees.
#[test]
fn a_channel_minted_outside_the_manager_has_no_count() {
    let mut manager = InterfaceManager::new(8);
    let minted = manager.new_channel(8);
    assert_eq!(manager.connected_streams(minted.address()), Some(0));

    let (rx_tx, _rx_rx) = InterfaceChannel::make_rx_channel(8);
    let (_tx_tx, tx_rx) = InterfaceChannel::make_tx_channel(8);
    let bare = InterfaceChannel::new(
        rx_tx,
        tx_rx,
        AddressHash::new_from_rand(OsRng),
        CancellationToken::new(),
    );
    assert_eq!(manager.connected_streams(bare.address()), None);
}
