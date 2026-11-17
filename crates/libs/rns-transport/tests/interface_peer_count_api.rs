//! The per-interface peer counter used from outside the crate:
//! `Transport::interface_peer_count` reads the AutoInterface discovery peer table length
//! behind an address. Every interface kind without such a table reads `None`: a plain
//! `TcpClient`, the shared instance's interface named by `shared_instance()`, an address
//! the transport never minted, and a channel built with `InterfaceChannel::new`. A channel
//! the manager minted reads `None` until `InterfaceManager::set_peer_counter` installs a
//! counter, then whatever the counter holds; a virtual child registered on that channel,
//! before or after the install, answers its host's count, and every read goes `None` once
//! the transport stops.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rns_transport::hash::{AddressHash, Hash};
use rns_transport::iface::{IfaceRole, InterfaceChannel, InterfaceManager};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport,
};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/close_orderly.rs"]
mod close_orderly;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/spawn_plain_client.rs"]
mod spawn_plain_client;
use accept_within::accept_within;
use client_transport::client_transport;
use close_orderly::close_orderly;
use free_loopback_port::free_loopback_port;
use spawn_plain_client::spawn_plain_client;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);

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

/// A plain `TcpClient` carries no peer table: `None` before and after the listener
/// accepts, as does an address the transport never minted.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_plain_tcp_client_has_no_peer_count() {
    let port = free_loopback_port().await;
    let endpoint: SocketAddr = format!("127.0.0.1:{port}").parse().expect("endpoint");
    let transport = client_transport("peer-count-plain");
    let (iface, _status) = spawn_plain_client(&transport, endpoint, RECONNECT_WAIT).await;

    assert_eq!(transport.interface_peer_count(&iface).await, None);
    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "the record exists; it carries no peer counter"
    );
    let unknown = AddressHash::new_from_hash(&Hash::new_from_slice(&[0xAAu8; 32]));
    assert_eq!(transport.interface_peer_count(&unknown).await, None);

    let listener = TcpListener::bind(endpoint).await.expect("bind the port the client dials");
    let stream = accept_within(&listener, "the connect").await;
    assert_eq!(transport.interface_peer_count(&iface).await, None);

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The shared instance's interface, named by `shared_instance()`, carries no peer table
/// either, once attached and once its connection is served.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_shared_instance_has_no_peer_count() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("peer-count-shared");
    let mut state = attach(&transport, endpoint).await;
    let iface = transport.shared_instance().expect("the slot names the attached interface");
    assert_eq!(transport.interface_peer_count(&iface).await, None);

    let stream = accept_within(&listener, "the connect").await;
    assert_eq!(next_state("the connect", &mut state).await, Some(SharedInstanceEvent::Connected));
    assert_eq!(transport.interface_peer_count(&iface).await, None);

    close_orderly(stream).await;
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A manager reads `None` for a channel built with `InterfaceChannel::new`, and for a
/// channel it minted until `set_peer_counter` installs a counter; from then on it reads
/// what the counter holds. `set_peer_counter` installs nothing for an unknown address.
#[test]
fn a_minted_channel_reads_its_counter_once_installed() {
    let mut manager = InterfaceManager::new(8);
    let minted = manager.new_channel(8);
    assert_eq!(manager.peer_count(minted.address()), None);

    let (rx_tx, _rx_rx) = InterfaceChannel::make_rx_channel(8);
    let (_tx_tx, tx_rx) = InterfaceChannel::make_tx_channel(8);
    let bare = InterfaceChannel::new(
        rx_tx,
        tx_rx,
        AddressHash::new_from_rand(OsRng),
        CancellationToken::new(),
    );
    assert_eq!(manager.peer_count(bare.address()), None);
    assert!(!manager.set_peer_counter(bare.address(), Arc::new(AtomicU64::new(7))));
    assert_eq!(manager.peer_count(bare.address()), None);

    let counter = Arc::new(AtomicU64::new(0));
    assert!(manager.set_peer_counter(minted.address(), Arc::clone(&counter)));
    assert_eq!(manager.peer_count(minted.address()), Some(0));
    counter.store(3, Ordering::Release);
    assert_eq!(manager.peer_count(minted.address()), Some(3));
}

/// A virtual child registered before the install and one registered after it both read
/// the host's count through the transport, and every read goes `None` once the transport
/// stops.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn virtual_children_share_the_host_count() {
    let transport = client_transport("peer-count-children");
    let manager = transport.iface_manager();
    let host = manager.lock().await.new_channel(8).address;
    let early_child = manager
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register a child before the install");
    assert_eq!(transport.interface_peer_count(&host).await, None);
    assert_eq!(transport.interface_peer_count(&early_child).await, None);

    let counter = Arc::new(AtomicU64::new(2));
    assert!(manager.lock().await.set_peer_counter(&host, Arc::clone(&counter)));
    let late_child = manager
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register a child after the install");

    for iface in [host, early_child, late_child] {
        assert_eq!(transport.interface_peer_count(&iface).await, Some(2));
    }
    counter.store(5, Ordering::Release);
    for iface in [host, early_child, late_child] {
        assert_eq!(transport.interface_peer_count(&iface).await, Some(5));
    }

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    for iface in [host, early_child, late_child] {
        assert_eq!(transport.interface_peer_count(&iface).await, None, "pruned after stop");
    }
}
