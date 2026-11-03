//! Usage probe for the `Transport::interface_connect_count` promise about the interfaces
//! that are not stream clients: "any other interface the manager spawned (listeners,
//! datagram, serial and KISS carriers, the I2P interface) reads `Some(0)` for as long as
//! it lives; only `TcpClient` and `LocalUnixClient` move the count". The listener half is
//! pinned by `interface_connect_count_probe.rs`; this file pins the datagram carrier: a
//! `UdpInterface` spawned through the transport reads `Some(0)` as soon as the manager
//! returns its address, still `Some(0)` once it has bound and handled a datagram, keeps
//! `Some(0)` while a sibling `TcpClient` on the same transport moves to `Some(1)`, and
//! reads `None` once `stop_interface` removed it.

use std::net::SocketAddr;
use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::udp::{UdpInterface, UdpRuntimeStatusHandle};
use rns_transport::transport::{InterfaceEventKind, Transport};
use tokio::net::{TcpListener, UdpSocket};
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/await_kind.rs"]
mod await_kind;
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
#[path = "support/wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use await_kind::await_kind;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;
use spawn_plain_client::spawn_plain_client;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);

/// A loopback UDP port the stack has just released.
#[deny(clippy::await_holding_invalid_type)]
async fn free_udp_port() -> u16 {
    let probe = UdpSocket::bind("127.0.0.1:0").await.expect("bind udp probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

/// Spawns a plain unicast `UdpInterface` bound to `bind` through `transport`'s interface
/// manager; returns its interface address and its status handle.
#[deny(clippy::await_holding_invalid_type)]
async fn spawn_udp_carrier(
    transport: &Transport,
    bind: SocketAddr,
    forward: Option<SocketAddr>,
) -> (AddressHash, UdpRuntimeStatusHandle) {
    let iface = UdpInterface::new(bind.to_string(), forward.map(|addr| addr.to_string()));
    let status = iface.runtime_status_handle();
    let address = transport.iface_manager().lock().await.spawn(iface, UdpInterface::spawn);
    (address, status)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_datagram_carrier_reads_zero_for_as_long_as_it_lives_and_none_once_stopped() {
    let bind: SocketAddr = format!("127.0.0.1:{}", free_udp_port().await).parse().expect("bind");
    let transport = client_transport("connect-count-udp-carrier");
    let unknown = AddressHash::new_from_rand(rns_transport::rand_core::OsRng);

    let (iface, status) = spawn_udp_carrier(&transport, bind, None).await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a datagram carrier the manager spawned reads Some(0) from the start"
    );
    assert_eq!(transport.interface_connect_count(&unknown).await, None);

    wait_until("the udp carrier to bind", || status.snapshot().link_state == "bound").await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "binding is not a connect"
    );

    let sender = UdpSocket::bind("127.0.0.1:0").await.expect("bind sender");
    let mut handled = 0;
    wait_until("the udp carrier to handle a datagram", || {
        if handled == 0 {
            // Not a Reticulum frame: the carrier counts it as a decode error, which is
            // enough to prove the rx task is alive and reading its socket. The send is
            // best-effort: a datagram the stack refuses or drops is sent again on the
            // next poll, and the assertion is on the carrier's counters.
            let _ = sender.try_send_to(b"not a reticulum packet", bind);
        }
        let snapshot = status.snapshot();
        handled = snapshot.packets_rx + snapshot.decode_errors;
        handled >= 1
    })
    .await;
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a datagram carrier that handled traffic still reads Some(0): only stream clients move the count"
    );

    assert!(transport.stop_interface(iface).await, "the spawned carrier stops");
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        None,
        "a removed carrier is unknown to the transport"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_datagram_carrier_keeps_zero_while_a_sibling_stream_client_connects() {
    let udp_bind: SocketAddr =
        format!("127.0.0.1:{}", free_udp_port().await).parse().expect("udp bind");
    let udp_peer: SocketAddr =
        format!("127.0.0.1:{}", free_udp_port().await).parse().expect("udp peer");
    let tcp_endpoint: SocketAddr =
        format!("127.0.0.1:{}", free_loopback_port().await).parse().expect("tcp endpoint");
    let transport = client_transport("connect-count-udp-beside-tcp");
    let mut events = transport.interface_events();

    let (udp_iface, udp_status) = spawn_udp_carrier(&transport, udp_bind, Some(udp_peer)).await;
    let (tcp_iface, _) = spawn_plain_client(&transport, tcp_endpoint, RECONNECT_WAIT).await;
    assert_eq!(transport.interface_connect_count(&udp_iface).await, Some(0));
    assert_eq!(transport.interface_connect_count(&tcp_iface).await, Some(0));

    let listener = TcpListener::bind(tcp_endpoint).await.expect("bind the port the client dials");
    let _stream = accept_within(&listener, "the tcp client's dial").await;
    await_kind("the tcp Connected", &mut events, tcp_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;
    wait_until("the udp carrier to bind", || udp_status.snapshot().link_state == "bound").await;

    assert_eq!(transport.interface_connect_count(&tcp_iface).await, Some(1));
    assert_eq!(
        transport.interface_connect_count(&udp_iface).await,
        Some(0),
        "a sibling stream client's connect does not move the datagram carrier's count"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(
        transport.interface_connect_count(&udp_iface).await,
        None,
        "stop prunes the carrier"
    );
    assert_eq!(transport.interface_connect_count(&tcp_iface).await, None, "stop prunes the client");
}
