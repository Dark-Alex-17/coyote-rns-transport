//! Usage probe for `Transport::path_interface` read the way the README's `## Driving
//! rnsd's path table` paragraph tells a consumer to use it, from outside the crate:
//!
//! - a transport with no interfaces at all answers `None` for an unknown destination and
//!   for a destination it registered itself, and `path_status` agrees (`path_found` false);
//! - the `iface` on the record is the address `InterfaceManager::spawn` returned for the
//!   `TcpClient`, the same tag `Transport::interface_events()` puts on that client's events,
//!   so the two surfaces can be joined by `iface` as the paragraph says;
//! - the recipe: the listener goes away, the connector reads `Disconnected` tagged with the
//!   path's interface, `expire_path` removes the record, `path_interface` reads `None`,
//!   `path_status` reads `path_found == false`, and `request_path` returns with the table
//!   still empty;
//! - a second announce of the same destination that the connector accepts moves
//!   `learned_at` forward to the second announce's bracket;
//! - an interface detached from the manager after the path was learned: the record is
//!   still returned with the same `iface`, `hops` and learn time, `name` reads `None`,
//!   `path_status` still finds the path, and `path_table` omits the row.
//!
//! Two transports on 127.0.0.1: a `TcpServer` listener and a `TcpClient` connector.

use std::time::{Duration, Instant, SystemTime};

use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, InterfaceEvent, InterfaceEventKind, Transport};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/learn_announce.rs"]
mod learn_announce;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;
use learn_announce::learn_announce;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const SECOND: Duration = Duration::from_secs(1);

struct Listener {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
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
async fn listener(name: &str) -> Listener {
    let transport = client_transport(name);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    Listener { transport, port, status }
}

/// The connecting side, spawned through the manager so the test holds the address the
/// manager minted for the client; the announce and interface-event receivers are
/// subscribed before the stream is up.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(
    name: &str,
    port: u16,
) -> (Transport, AddressHash, broadcast::Receiver<AnnounceEvent>, broadcast::Receiver<InterfaceEvent>)
{
    let transport = client_transport(name);
    let announces = transport.recv_announces().await;
    let events = transport.interface_events();
    let client = TcpClient::new(format!("127.0.0.1:{port}")).with_reconnect_wait(RECONNECT_WAIT);
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    wait_until("the connector to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, iface, announces, events)
}

/// The first event tagged `iface` whose kind satisfies `accept`.
#[deny(clippy::await_holding_invalid_type)]
async fn await_event(
    what: &str,
    events: &mut broadcast::Receiver<InterfaceEvent>,
    iface: AddressHash,
    accept: impl Fn(InterfaceEventKind) -> bool,
) -> InterfaceEventKind {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.iface() == iface && accept(event.kind()) => return event.kind(),
                Ok(_) | Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: the interface event stream closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what} timed out"))
}

fn within_bracket(at: SystemTime, before: SystemTime, after: SystemTime) -> bool {
    before - SECOND <= at && at <= after + SECOND
}

/// Cold start: a transport with no interfaces holds no paths, so an unknown destination
/// and one it registered itself both answer `None`, and `path_status` agrees.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_transport_with_no_interfaces_answers_none_and_agrees_with_path_status() {
    let transport = client_transport("path-interface-cold");
    let unknown = AddressHash::new_from_rand(OsRng);
    assert_eq!(transport.path_interface(&unknown).await, None);
    assert!(!transport.path_status(&unknown).await.path_found);

    let local = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "local"),
        )
        .await;
    let local_hash = local.lock().await.desc.address_hash;
    assert_eq!(
        transport.path_interface(&local_hash).await,
        None,
        "a destination of this transport's own is not a path table entry"
    );
    assert!(!transport.path_status(&local_hash).await.path_found);
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// The README recipe end to end. The record's `iface` is the address `spawn` returned
/// for the `TcpClient`, the same tag the interface event stream carries; when the
/// listener goes away the stream reads `Disconnected` under that tag, `expire_path`
/// removes the record, and `request_path` returns on the now-pathless destination with
/// the table still empty.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_record_joins_interface_events_by_iface_and_the_expire_then_request_recipe_holds() {
    let listener = listener("path-interface-recipe-listener").await;
    let (connector, client_iface, mut announces, mut events) =
        connector("path-interface-recipe-connector", listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    await_event("the client's Connected", &mut events, client_iface, |kind| {
        kind == InterfaceEventKind::Connected
    })
    .await;

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "echo"),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce("the announce", &mut announces, &hash, WAIT).await;

    let path = connector.path_interface(&hash).await.expect("the path is on file");
    assert_eq!(
        path.iface, client_iface,
        "the record names the interface the manager minted for the client, the tag interface_events uses"
    );

    // The listener stops: its accepted peer stream ends, the connector's client reads the
    // end of its stream and publishes Disconnected under the path's interface.
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
    let kind = await_event("the client's Disconnected", &mut events, path.iface, |kind| {
        matches!(kind, InterfaceEventKind::Disconnected { .. })
    })
    .await;
    assert!(matches!(kind, InterfaceEventKind::Disconnected { .. }), "{kind:?}");

    // The interface is down: expire, then ask again.
    assert!(connector.expire_path(&hash).await, "the entry is removed");
    assert_eq!(connector.path_interface(&hash).await, None);
    assert!(!connector.path_status(&hash).await.path_found, "path_status agrees");
    assert!(!connector.expire_path(&hash).await, "expiring twice removes nothing more");
    let _trace = connector.request_path(&hash, None, None).await;
    assert_eq!(connector.path_interface(&hash).await, None, "no answer, no entry");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
}

/// `learned_at` is the time this transport accepted the announce: a second announce of
/// the same destination, accepted a little later, moves `learned_at` into the second
/// bracket while `iface` and `hops` stay put.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_reannounce_moves_learned_at_forward() {
    let listener = listener("path-interface-relearn-listener").await;
    let (connector, client_iface, mut announces, _events) =
        connector("path-interface-relearn-connector", listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "again"),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;

    let first_before = SystemTime::now();
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce("the first announce", &mut announces, &hash, WAIT).await;
    let first_after = SystemTime::now();
    let first = connector.path_interface(&hash).await.expect("the path is on file");
    assert!(
        within_bracket(first.learned_at, first_before, first_after),
        "learned_at {:?} lies within a second of the first announce bracket {first_before:?}..={first_after:?}",
        first.learned_at
    );

    // Python keys the emission on whole seconds; cross one so the second announce is new.
    tokio::time::sleep(Duration::from_millis(1100)).await;

    let second_before = SystemTime::now();
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce("the second announce", &mut announces, &hash, WAIT).await;
    let second_after = SystemTime::now();
    let second = connector.path_interface(&hash).await.expect("the path is still on file");

    assert_eq!(second.iface, client_iface);
    assert_eq!(second.hops, first.hops);
    assert!(
        second.learned_at > first.learned_at + Duration::from_millis(500),
        "learned_at follows the accepted announce: first {:?}, second {:?}",
        first.learned_at,
        second.learned_at
    );
    assert!(
        within_bracket(second.learned_at, second_before, second_after),
        "learned_at {:?} lies within a second of the second announce bracket {second_before:?}..={second_after:?}",
        second.learned_at
    );

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

/// The README's detached-interface clause. The connector names its client `uplink`,
/// learns a path over it, then stops the interface through the manager: the record is
/// still returned by `path_interface` with the same `iface`, `hops` and learn time,
/// `name` is `None` now that the manager no longer registers the interface,
/// `path_status` still finds the path, and `path_table` no longer lists the row.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_detached_interface_keeps_the_record_drops_its_name_and_leaves_path_table() {
    let listener = listener("path-interface-detach-listener").await;
    let (connector, client_iface, mut announces, _events) =
        connector("path-interface-detach-connector", listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    assert!(connector
        .iface_manager()
        .lock()
        .await
        .set_display_name(client_iface, Some("uplink".to_owned())));

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "detach"),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce("the announce", &mut announces, &hash, WAIT).await;

    let attached = connector.path_interface(&hash).await.expect("the path is on file");
    assert_eq!(attached.iface, client_iface);
    assert_eq!(attached.name.as_deref(), Some("uplink"), "the manager's display name");
    let rows = connector.path_table(None).await.expect("path table");
    let row = rows.iter().find(|row| row.destination == hash).expect("the table lists the path");
    assert_eq!(row.interface_name.as_deref(), Some("uplink"), "path_table reads the same name");

    // Detach the interface the path rests on; the manager forgets it.
    assert!(connector.iface_manager().lock().await.stop_interface(client_iface));
    assert!(
        !connector.iface_manager().lock().await.interface_hashes().contains(&client_iface),
        "the manager no longer registers the detached interface"
    );
    // Let any reaction to the stop settle before reading, so the read is of the steady
    // state, while staying well inside the 10 s cleanup interval that would cull the entry.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let detached = connector
        .path_interface(&hash)
        .await
        .expect("a path whose interface has since been detached is still returned");
    assert_eq!(detached.iface, client_iface, "the interface the announce arrived on");
    assert_eq!(detached.hops, attached.hops);
    assert_eq!(detached.name, None, "no longer registered with the manager, no name");
    let drift = detached
        .learned_at
        .duration_since(attached.learned_at)
        .unwrap_or_else(|earlier| earlier.duration());
    assert!(drift < Duration::from_millis(50), "the learn time did not move: {drift:?}");
    assert!(
        connector.path_status(&hash).await.path_found,
        "Some iff the table has an entry: path_status still finds the path"
    );
    let rows = connector.path_table(None).await.expect("path table");
    assert!(
        rows.iter().all(|row| row.destination != hash),
        "path_table omits a path whose interface is no longer registered"
    );

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}
