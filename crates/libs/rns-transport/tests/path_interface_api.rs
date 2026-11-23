//! `Transport::path_interface` and `PathIface` read from outside the crate: a listener
//! with a `TcpServer` on 127.0.0.1 announces `example/echo`, a connector with a
//! `TcpClient` learns the announce and reads the path's interface off the record by
//! field. The interface and hops agree with `path_status` and `hops_to`, `learned_at`
//! lies within a second of the announce, `name` is `None` until the interface manager
//! is given a display name and that name afterwards, an unknown destination answers
//! `None`, and `expire_path` removes the record. `PathIface` is a `Send + Sync` value
//! type: `Debug`, `Clone`, `PartialEq` and `Eq`.

use std::time::{Duration, Instant, SystemTime};

use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, PathIface, Transport, TransportConfig};
use tokio::sync::broadcast;
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;
#[path = "support/learn_announce.rs"]
mod learn_announce;
use learn_announce::learn_announce;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// The slack either side of the announce bracket that `learned_at` must fall within.
const SECOND: Duration = Duration::from_secs(1);

/// The announcing side: a transport with a `TcpServer` on 127.0.0.1.
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
async fn listener() -> Listener {
    let transport = Transport::new(TransportConfig::new(
        "path-interface-listener",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
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

/// The connecting side: a transport with a `TcpClient` on the listener's port. The
/// announce receiver is subscribed before the stream is up so the announce cannot be missed.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(port: u16) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport = Transport::new(TransportConfig::new(
        "path-interface-connector",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until("the connector to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
}

#[test]
fn path_iface_is_a_send_sync_value_type() {
    fn is_send_sync<T: Send + Sync>() {}
    fn is_value<T: std::fmt::Debug + Clone + PartialEq + Eq>() {}
    is_send_sync::<PathIface>();
    is_value::<PathIface>();
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn path_interface_exposes_the_interface_its_name_the_learn_time_and_the_hops() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    // The accepted child interface that carries the listener's traffic is registered a
    // task wake after the connector reports its stream connected.
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;

    let before = SystemTime::now();
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce("the announce", &mut announces, &destination_hash, WAIT).await;
    let after = SystemTime::now();

    let path: PathIface =
        connector.path_interface(&destination_hash).await.expect("the path is on file");
    // Pins the public fields' types as seen from outside the crate.
    let iface: AddressHash = path.iface;
    let name: Option<String> = path.name.clone();
    let learned_at: SystemTime = path.learned_at;
    let hops: u8 = path.hops;

    let status = connector.path_status(&destination_hash).await;
    assert_eq!(Some(iface), status.interface, "the same interface path_status reports");
    assert_eq!(hops, connector.hops_to(&destination_hash).await);
    assert_eq!(hops, 1, "the announce came straight from the listener");
    assert!(
        before - SECOND <= learned_at && learned_at <= after + SECOND,
        "learned_at {learned_at:?} lies within a second of the announce bracket {before:?}..={after:?}"
    );
    assert_eq!(name, None, "the TcpClient has no display name");

    assert!(connector
        .iface_manager()
        .lock()
        .await
        .set_display_name(iface, Some("uplink".to_owned())));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let named = connector.path_interface(&destination_hash).await.expect("the path is on file");
    assert_eq!(named.name.as_deref(), Some("uplink"));
    assert_eq!(named.iface, iface);
    // Two reads of one entry convert the same `Instant` through two clock samples,
    // so they agree to the sampling imprecision, not exactly; the sleep makes a
    // relearn visible as a shift of its own size.
    let drift =
        named.learned_at.duration_since(learned_at).unwrap_or_else(|earlier| earlier.duration());
    assert!(
        drift < Duration::from_millis(50),
        "two reads of one entry agree to the clock-sampling imprecision; a relearn would move learned_at by the 100 ms sleep: the reads differ by {drift:?}"
    );
    assert_eq!(named.clone(), named, "the record compares by value");

    assert_eq!(
        connector.path_interface(&AddressHash::new_from_rand(OsRng)).await,
        None,
        "no path table entry, no interface"
    );
    assert!(connector.expire_path(&destination_hash).await, "the path table entry is removed");
    assert_eq!(connector.path_interface(&destination_hash).await, None);

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}
