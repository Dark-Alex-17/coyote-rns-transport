//! Usage probes for `Transport::interface_connect_count` beyond the plain client, the
//! shared instance and the unix client that `interface_connect_count_api.rs` covers:
//! a `TcpServer` runs each accepted peer as a client over a pre-supplied stream, and
//! that peer's own record counts the stream it was handed while the listening
//! interface's record stays at zero, and a `LocalUnixServer`'s accepted peer does the
//! same with no event published; a virtual interface registered on a host reads the
//! host's count; two clients spawned through one transport keep separate counts; the
//! count survives a disconnect and a refused redial unchanged; and an interface that
//! `stop_interface` removed reads `None` while a status handle that outlives it keeps
//! the last count.

use std::net::SocketAddr;
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixEndpoint, LocalUnixServer};
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::iface::IfaceRole;
use rns_transport::transport::InterfaceEventKind;
use tokio::net::{TcpListener, TcpStream};
#[cfg(unix)]
use tokio::sync::broadcast::error::TryRecvError;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/await_kind.rs"]
mod await_kind;
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/close_orderly.rs"]
mod close_orderly;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/wait_for_count.rs"]
mod wait_for_count;
#[path = "support/wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use await_kind::await_kind;
use client_transport::client_transport;
use close_orderly::close_orderly;
use free_loopback_port::free_loopback_port;
use spawn_plain_client::spawn_plain_client;
use wait_for_count::wait_for_count;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
#[cfg(unix)]
const POLL: Duration = Duration::from_millis(20);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
#[cfg(unix)]
const QUIET: Duration = Duration::from_millis(300);

/// The accepted peer's interface the server's status names, parsed back from the
/// `latest_client_iface` display form `/<hex>/`.
fn latest_client_iface(server_status: &serde_json::Value) -> AddressHash {
    let text = server_status["latest_client_iface"]
        .as_str()
        .expect("the server names its latest accepted peer");
    AddressHash::new_from_hex_string(text.trim_matches('/'))
        .expect("the latest_client_iface display form parses back")
}

/// A `TcpServer` spawned through a transport runs each accepted peer as a `TcpClient`
/// over a pre-supplied stream. Each peer's own record counts the one stream it was
/// handed, the listening interface's record never moves off zero, and the server's
/// status JSON carries the peer's count under `latest_stream_status`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_tcp_server_counts_each_accepted_peer_on_its_own_record() {
    let server_transport = client_transport("connect-count-server");
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), server_transport.iface_manager());
    let server_status = server.runtime_status_handle();
    let server_iface =
        server_transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        server_status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    assert_eq!(server_transport.interface_connect_count(&server_iface).await, Some(0));

    let first = TcpStream::connect(("127.0.0.1", port)).await.expect("the first peer connects");
    wait_until("the first accept", || {
        server_status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let first_iface = latest_client_iface(&server_status.to_json());
    wait_for_count("the first peer's stream", &server_transport, first_iface, 1).await;
    assert_eq!(
        server_status.to_json()["latest_stream_status"]["connected_streams"].as_u64(),
        Some(1),
        "the server's status carries the accepted peer's count"
    );

    let second = TcpStream::connect(("127.0.0.1", port)).await.expect("the second peer connects");
    wait_until("the second accept", || {
        server_status.to_json()["accepted_connections"].as_u64() == Some(2)
    })
    .await;
    let second_iface = latest_client_iface(&server_status.to_json());
    assert_ne!(second_iface, first_iface, "each accepted peer has its own interface");
    wait_for_count("the second peer's stream", &server_transport, second_iface, 1).await;
    assert_eq!(
        server_transport.interface_connect_count(&first_iface).await,
        Some(1),
        "the second accept does not touch the first peer's record"
    );
    assert_eq!(
        server_transport.interface_connect_count(&server_iface).await,
        Some(0),
        "the listening interface itself establishes no stream"
    );

    close_orderly(first).await;
    close_orderly(second).await;
    timeout(WAIT, server_transport.stop()).await.expect("transport stop completes");
}

/// The unix twin of the test above. A `LocalUnixServer` has no status handle, so the
/// accepted peer is found as the one local-client child the manager lists once the
/// listener is flagged as the shared instance. The peer's record counts the stream it
/// was handed, the listener's record stays at `Some(0)`, and a subscriber that predates
/// the spawn reads nothing: the pre-supplied stream publishes no `Connected`.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_unix_server_counts_each_accepted_peer_on_its_own_record() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("connect-count.sock");
    let transport = client_transport("connect-count-unix-server");
    let mut events = transport.interface_events();
    let server = LocalUnixServer::new_endpoint(
        LocalUnixEndpoint::filesystem(&socket),
        transport.iface_manager(),
    );
    let server_iface = transport.iface_manager().lock().await.spawn(server, LocalUnixServer::spawn);
    assert!(
        transport.iface_manager().lock().await.set_shared_instance(server_iface, true),
        "the listener is flagged as the shared instance"
    );
    wait_until("the socket to appear", || socket.exists()).await;
    assert_eq!(transport.interface_connect_count(&server_iface).await, Some(0));

    let stream = tokio::net::UnixStream::connect(&socket).await.expect("the peer connects");
    let deadline = Instant::now() + WAIT;
    let peer = loop {
        let children = transport.iface_manager().lock().await.local_client_interfaces();
        if let [peer] = children[..] {
            break peer;
        }
        assert!(Instant::now() < deadline, "timed out waiting for the accepted peer: {children:?}");
        tokio::time::sleep(POLL).await;
    };
    assert_ne!(peer, server_iface);
    wait_for_count("the peer's stream", &transport, peer, 1).await;
    assert_eq!(
        transport.interface_connect_count(&server_iface).await,
        Some(0),
        "the listening interface itself establishes no stream"
    );

    tokio::time::sleep(QUIET).await;
    assert_eq!(
        events.try_recv(),
        Err(TryRecvError::Empty),
        "neither the listener nor the accepted peer publishes"
    );

    close_orderly(stream).await;
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A virtual interface registered on a client's interface shares the host's count: it
/// reads `Some(0)` beside the host before the connect and follows the host to `Some(1)`
/// and `Some(2)` across a connect and a redial.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_virtual_interface_reads_its_hosts_count() {
    let port = free_loopback_port().await;
    let endpoint: SocketAddr = format!("127.0.0.1:{port}").parse().expect("endpoint");
    let transport = client_transport("connect-count-virtual");
    let mut events = transport.interface_events();
    let (host, _status) = spawn_plain_client(&transport, endpoint, RECONNECT_WAIT).await;
    let child = transport
        .iface_manager()
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("a virtual interface on a spawned host");
    assert_ne!(child, host);
    assert_eq!(transport.interface_connect_count(&child).await, Some(0));

    let listener = TcpListener::bind(endpoint).await.expect("bind the port the host dials");
    let stream = accept_within(&listener, "the first connect").await;
    await_kind("the first Connected", &mut events, host, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    assert_eq!(transport.interface_connect_count(&host).await, Some(1));
    assert_eq!(transport.interface_connect_count(&child).await, Some(1));

    close_orderly(stream).await;
    let stream = accept_within(&listener, "the redial").await;
    await_kind("the second Connected", &mut events, host, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    assert_eq!(transport.interface_connect_count(&host).await, Some(2));
    assert_eq!(transport.interface_connect_count(&child).await, Some(2));

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// Two clients spawned through one transport keep separate counts: the one whose port
/// is served reads `Some(1)` then `Some(2)` while its sibling, dialing a port nothing
/// listens on, stays at `Some(0)` through both getters.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn sibling_clients_keep_separate_counts() {
    let served_port = free_loopback_port().await;
    let served: SocketAddr = format!("127.0.0.1:{served_port}").parse().expect("endpoint");
    let refused_port = free_loopback_port().await;
    let refused: SocketAddr = format!("127.0.0.1:{refused_port}").parse().expect("endpoint");
    let transport = client_transport("connect-count-siblings");
    let mut events = transport.interface_events();
    let (served_iface, served_status) =
        spawn_plain_client(&transport, served, RECONNECT_WAIT).await;
    let (refused_iface, refused_status) =
        spawn_plain_client(&transport, refused, RECONNECT_WAIT).await;
    assert_ne!(served_iface, refused_iface);

    let listener = TcpListener::bind(served).await.expect("bind the served port");
    let stream = accept_within(&listener, "the served connect").await;
    await_kind("the served Connected", &mut events, served_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    assert_eq!(transport.interface_connect_count(&served_iface).await, Some(1));
    assert_eq!(served_status.connected_streams(), 1);
    assert_eq!(transport.interface_connect_count(&refused_iface).await, Some(0));
    assert_eq!(refused_status.connected_streams(), 0);

    close_orderly(stream).await;
    let stream = accept_within(&listener, "the served redial").await;
    await_kind("the second served Connected", &mut events, served_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    assert_eq!(transport.interface_connect_count(&served_iface).await, Some(2));
    assert_eq!(
        transport.interface_connect_count(&refused_iface).await,
        Some(0),
        "the sibling's redial does not touch this record"
    );
    assert_eq!(refused_status.to_json()["connected_streams"].as_u64(), Some(0));

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The count is the number of streams established, not the number currently open: it
/// stays `Some(1)` through the disconnect and a refused redial. Once `stop_interface`
/// removes the interface the transport reads `None` for it, while a status handle taken
/// before the stop keeps reading the last count.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_stopped_interface_reads_none_while_its_handle_keeps_the_last_count() {
    let port = free_loopback_port().await;
    let endpoint: SocketAddr = format!("127.0.0.1:{port}").parse().expect("endpoint");
    let transport = client_transport("connect-count-stopped");
    let mut events = transport.interface_events();
    let (iface, status) = spawn_plain_client(&transport, endpoint, RECONNECT_WAIT).await;

    let listener = TcpListener::bind(endpoint).await.expect("bind the port the client dials");
    let stream = accept_within(&listener, "the connect").await;
    await_kind("the Connected", &mut events, iface, |kind| kind == InterfaceEventKind::Connected)
        .await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));

    drop(listener);
    close_orderly(stream).await;
    await_kind("the Disconnected", &mut events, iface, |kind| {
        matches!(kind, InterfaceEventKind::Disconnected { .. })
    })
    .await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(1),
        "a disconnect does not move the count"
    );
    await_kind("the refused redial", &mut events, iface, |kind| {
        matches!(kind, InterfaceEventKind::DialFailed { .. })
    })
    .await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(1),
        "a refused redial does not move the count"
    );
    assert_eq!(status.connected_streams(), 1);

    assert!(transport.stop_interface(iface).await, "the spawned interface stops");
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        None,
        "a removed interface is unknown to the transport"
    );
    assert_eq!(status.connected_streams(), 1, "the handle outlives the record");
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
