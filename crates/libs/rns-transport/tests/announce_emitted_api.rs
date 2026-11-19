//! `AnnounceEvent::emitted`, `AnnounceEvent::random_blob` and
//! `Transport::latest_announce_emitted` read from outside the crate: a listener with a
//! `TcpServer` on 127.0.0.1 announces `example/echo`, a connector with a `TcpClient` learns
//! the announce and reads the emission second off the event. The field is bracketed by
//! clock reads around the announce, the blob's last five bytes encode it big-endian, the
//! accessor returns the same second for the learned address and `None` for an unknown one,
//! and a re-announce a second later is strictly newer under a different blob. The accessor
//! is path-table state: `deregister_destination` leaves it standing, `expire_path` removes it.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rns_transport::destination::{DestinationName, RAND_HASH_LENGTH};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, Transport, TransportConfig};
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
/// Equal hops need a strictly newer emission second before a re-announce replaces the path
/// on file (`RNS/Transport.py:2238`), so the second announce waits out the first's second.
const NEXT_SECOND: Duration = Duration::from_millis(1100);

/// The announcing side: a transport with a `TcpServer` on 127.0.0.1.
struct Listener {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
}

fn clock_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after the epoch").as_secs()
}

/// Bytes 5..10 of the blob, big-endian (`RNS/Transport.py:2217`).
fn blob_emitted(random_blob: &[u8; RAND_HASH_LENGTH]) -> u64 {
    let mut emitted = [0u8; 8];
    emitted[3..].copy_from_slice(&random_blob[5..]);
    u64::from_be_bytes(emitted)
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
        "announce-emitted-listener",
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
        "announce-emitted-connector",
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn announce_event_exposes_the_emission_second_and_the_blob_that_encodes_it() {
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

    let before = clock_secs();
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    let first_done = Instant::now();
    let after = clock_secs();
    let first = learn_announce("first announce", &mut announces, &destination_hash, WAIT).await;

    assert!(
        before <= first.emitted && first.emitted <= after,
        "emitted {} lies in the clock bracket {before}..={after}",
        first.emitted
    );
    // Pins the public field's type as seen from outside the crate.
    let blob: [u8; RAND_HASH_LENGTH] = first.random_blob;
    assert_eq!(blob_emitted(&blob), first.emitted, "the blob encodes emitted");
    assert_eq!(
        connector.latest_announce_emitted(&destination_hash).await,
        Some(first.emitted),
        "the path table holds the one blob the announce carried"
    );
    assert_eq!(
        connector.latest_announce_emitted(&AddressHash::new_from_rand(OsRng)).await,
        None,
        "no path table entry, no emission second"
    );

    tokio::time::sleep_until((first_done + NEXT_SECOND).into()).await;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    let second = learn_announce("re-announce", &mut announces, &destination_hash, WAIT).await;

    assert!(
        second.emitted > first.emitted,
        "re-announce emitted {} is newer than {}",
        second.emitted,
        first.emitted
    );
    assert_ne!(second.random_blob, first.random_blob, "a fresh announce draws a fresh blob");
    assert_eq!(blob_emitted(&second.random_blob), second.emitted);
    assert_eq!(
        connector.latest_announce_emitted(&destination_hash).await,
        Some(second.emitted),
        "the newest blob on file is the re-announce"
    );

    assert!(connector.deregister_destination(&destination_hash).await, "the desc is removed");
    assert_eq!(
        connector.latest_announce_emitted(&destination_hash).await,
        Some(second.emitted),
        "dropping the desc leaves the path table entry standing"
    );
    assert!(connector.expire_path(&destination_hash).await, "the path table entry is removed");
    assert_eq!(connector.latest_announce_emitted(&destination_hash).await, None);

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}
