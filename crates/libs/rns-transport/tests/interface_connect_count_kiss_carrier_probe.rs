//! Usage probe for the KISS clause of the `Transport::interface_connect_count` rule: "Any
//! other interface the manager spawned (listeners, datagram, serial and KISS carriers, the
//! I2P interface) reads `Some(0)` for as long as it lives; only `TcpClient` and
//! `LocalUnixClient` move the count." A `KissTcpClientInterface` is the one KISS carrier that
//! needs no hardware, and it is the sharpest case of the rule: it dials a TCP endpoint and
//! establishes real streams, yet it is not a `TcpClient`, so its established streams must not
//! move the count. The probe spawns one through a transport against a raw listener, serves it
//! a first stream and a redial, lets a sibling `TcpClient` on the same transport move to
//! `Some(1)`, and reads `Some(0)` for the KISS carrier throughout; `stop_interface` then
//! takes it to `None`.

use std::net::SocketAddr;
use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::kiss::{KissRuntimeStatusHandle, KissTcpClientInterface};
use rns_transport::transport::{InterfaceEventKind, Transport};
use tokio::net::TcpListener;
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
#[path = "support/wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use await_kind::await_kind;
use client_transport::client_transport;
use close_orderly::close_orderly;
use free_loopback_port::free_loopback_port;
use spawn_plain_client::spawn_plain_client;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);

/// Whether a KISS carrier's status says its stream is up: `connected` right after the dial,
/// `running` once its framing loop has taken the stream over.
fn stream_is_up(status: &KissRuntimeStatusHandle) -> bool {
    matches!(status.snapshot().link_state.as_str(), "connected" | "running")
}

/// Spawns a `KissTcpClientInterface` dialing `endpoint` through `transport`'s interface
/// manager with a short reconnect backoff; returns its interface address and status handle.
#[deny(clippy::await_holding_invalid_type)]
async fn spawn_kiss_tcp_carrier(
    transport: &Transport,
    endpoint: SocketAddr,
) -> (AddressHash, KissRuntimeStatusHandle) {
    let carrier =
        KissTcpClientInterface::new(endpoint.to_string()).with_reconnect_backoff(RECONNECT_WAIT);
    let status = carrier.runtime_status_handle();
    let iface =
        transport.iface_manager().lock().await.spawn(carrier, KissTcpClientInterface::spawn);
    (iface, status)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_kiss_tcp_carrier_reads_zero_across_its_streams_and_none_once_stopped() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind the kiss listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("connect-count-kiss-carrier");

    let (iface, status) = spawn_kiss_tcp_carrier(&transport, endpoint).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a KISS carrier the manager spawned reads Some(0) from the start"
    );

    let stream = accept_within(&listener, "the kiss carrier's first dial").await;
    wait_until("the kiss carrier to report its stream up", || stream_is_up(&status)).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a KISS carrier's established stream does not move the count: it is not a TcpClient"
    );

    close_orderly(stream).await;
    let redial = accept_within(&listener, "the kiss carrier's redial").await;
    // The redial's accept proves the second stream; the status may still read the first
    // stream's close for a moment, so wait for it to come back up before reading.
    wait_until("the kiss carrier to report its second stream up", || stream_is_up(&status)).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a KISS carrier's redial does not move the count either"
    );

    assert!(transport.stop_interface(iface).await, "the spawned carrier stops");
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        None,
        "a removed carrier is unknown to the transport"
    );
    drop(redial);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_kiss_tcp_carrier_keeps_zero_while_a_sibling_tcp_client_moves_to_one() {
    let kiss_listener = TcpListener::bind("127.0.0.1:0").await.expect("bind the kiss listener");
    let kiss_endpoint = kiss_listener.local_addr().expect("kiss listener address");
    let tcp_endpoint: SocketAddr =
        format!("127.0.0.1:{}", free_loopback_port().await).parse().expect("tcp endpoint");
    let transport = client_transport("connect-count-kiss-beside-tcp");
    let mut events = transport.interface_events();

    let (kiss_iface, kiss_status) = spawn_kiss_tcp_carrier(&transport, kiss_endpoint).await;
    let (tcp_iface, tcp_status) =
        spawn_plain_client(&transport, tcp_endpoint, RECONNECT_WAIT).await;
    assert_ne!(kiss_iface, tcp_iface);
    assert_eq!(transport.interface_connect_count(&kiss_iface).await, Some(0));
    assert_eq!(transport.interface_connect_count(&tcp_iface).await, Some(0));

    let _kiss_stream = accept_within(&kiss_listener, "the kiss carrier's dial").await;
    wait_until("the kiss carrier to report its stream up", || stream_is_up(&kiss_status)).await;

    let tcp_listener = TcpListener::bind(tcp_endpoint).await.expect("bind the tcp port");
    let _tcp_stream = accept_within(&tcp_listener, "the tcp client's dial").await;
    await_kind("the tcp Connected", &mut events, tcp_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;

    assert_eq!(transport.interface_connect_count(&tcp_iface).await, Some(1));
    assert_eq!(tcp_status.to_json()["connected_streams"].as_u64(), Some(1));
    assert_eq!(
        transport.interface_connect_count(&kiss_iface).await,
        Some(0),
        "only TcpClient and LocalUnixClient move the count; the connected KISS carrier stays at 0"
    );
    assert!(stream_is_up(&kiss_status), "the KISS carrier's stream is up while it reads Some(0)");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(
        transport.interface_connect_count(&kiss_iface).await,
        None,
        "stop prunes the carrier"
    );
    assert_eq!(transport.interface_connect_count(&tcp_iface).await, None, "stop prunes the client");
}
