//! `AnnounceEvent::emitted`, `AnnounceEvent::random_blob` and
//! `Transport::latest_announce_emitted` read by a peer that joins after the destination
//! announced: it never hears the original, and the first announce it gets is the relay's
//! path response, so without the emission second a replay would look like a fresh
//! announce. What that peer can rely on:
//!
//! - the relayed path response carries the emission second of the original announce, which
//!   is strictly older than the clock at request time once a second has passed, and the
//!   same blob the relay itself filed when it heard the announce (`RNS/Transport.py:3459-3473`);
//! - `latest_announce_emitted` on the late joiner is `None` before the response and the
//!   original second after it;
//! - replaying is idempotent: expire, ask again, same blob and second;
//! - a path response whose blob is already on file at equal hops and an equal second is not
//!   a newer path (`RNS/Transport.py:2238`), so it does not surface as a second event;
//! - after the destination re-announces in a later second, the relay replays the newest
//!   announce, and every transport's accessor follows the max.
//!
//! Three transports on 127.0.0.1: a transport-enabled `TcpServer` relay with
//! `announce_retry_limit(0)`, and two `TcpClient` connectors A (announcer) and B (late joiner).

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rns_transport::destination::{DestinationName, RAND_HASH_LENGTH};
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
#[path = "support/wait_until_within.rs"]
mod wait_until_within;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
/// Long enough for a relay on loopback to answer a path request many times over; used only
/// for the negative "no second event" check.
const QUIET: Duration = Duration::from_secs(2);
/// Equal hops need a strictly newer emission second (`RNS/Transport.py:2238`).
const NEXT_SECOND: Duration = Duration::from_millis(1100);
/// The relay's single rebroadcast fires on the first 1 s announce-retransmit tick after a
/// deadline drawn from 0..=500 ms, so it has landed by about 1.5 s after the announce. The
/// late joiner attaches only after that, so it can only ever hear a path response.
const PAST_REBROADCAST: Duration = Duration::from_millis(1700);

struct Relay {
    transport: Transport,
    announces: broadcast::Receiver<AnnounceEvent>,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
}

fn clock_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after the epoch").as_secs()
}

/// Bytes 5..10 of the blob, big-endian (`RNS/Transport.py:3725-3726`).
fn blob_emitted(random_blob: &[u8; RAND_HASH_LENGTH]) -> u64 {
    let mut emitted = [0u8; 8];
    emitted[3..].copy_from_slice(&random_blob[5..]);
    u64::from_be_bytes(emitted)
}

/// A transport-enabled listener that rebroadcasts each announce exactly once.
#[deny(clippy::await_holding_invalid_type)]
async fn relay() -> Relay {
    let mut config = TransportConfig::new(
        "announce-emitted-probe-relay",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    );
    config.set_transport_enabled(true);
    config.set_announce_retry_limit(0);
    let transport = Transport::new(config);
    let announces = transport.recv_announces().await;
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until_within("the relay to listen", WAIT, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    Relay { transport, announces, port, status }
}

/// A `TcpClient` connector whose announce receiver exists before the stream is up.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(name: &str, port: u16) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until_within(&format!("{name} to connect"), WAIT, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
}

#[deny(clippy::await_holding_invalid_type)]
async fn await_accepted(relay: &Relay, count: u64) {
    wait_until_within(&format!("the relay to accept {count}"), WAIT, || {
        relay.status.to_json()["accepted_connections"].as_u64() == Some(count)
    })
    .await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn late_joiner_hears_the_original_emission_in_the_relayed_path_response() {
    let mut relay = relay().await;
    let (a, _a_announces) = connector("announce-emitted-probe-a", relay.port).await;
    await_accepted(&relay, 1).await;

    let destination = a
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "announce-emitted-late"),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;

    // A announces while B is not attached. The relay files the blob.
    let before = clock_secs();
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    a.send_packet(packet).await;
    let announced_at = Instant::now();
    let after = clock_secs();
    let at_relay = learn_announce("relay hears A", &mut relay.announces, &hash, WAIT).await;
    assert!(!at_relay.is_path_response);
    assert!(
        before <= at_relay.emitted && at_relay.emitted <= after,
        "relay-side emitted {} lies in {before}..={after}",
        at_relay.emitted
    );
    assert_eq!(blob_emitted(&at_relay.random_blob), at_relay.emitted);
    assert_eq!(relay.transport.latest_announce_emitted(&hash).await, Some(at_relay.emitted));

    // Wait out the relay's one rebroadcast so B can only hear a path response; this also
    // moves into a later second, so "replay time" and "emission time" are distinguishable.
    tokio::time::sleep_until((announced_at + PAST_REBROADCAST).into()).await;
    let (b, mut b_announces) = connector("announce-emitted-probe-b", relay.port).await;
    await_accepted(&relay, 2).await;
    assert_eq!(b.latest_announce_emitted(&hash).await, None, "B never heard A");

    let request_clock = clock_secs();
    assert!(request_clock > at_relay.emitted, "the clock moved past the emission second");
    let trace = b.request_path(&hash, None, None).await;
    assert!(trace.sent_ifaces >= 1, "B's path request left on an interface: {trace:?}");
    let replayed = learn_announce("B's first path response", &mut b_announces, &hash, WAIT).await;

    assert!(replayed.is_path_response, "B's first announce for A is the relay's answer");
    assert_eq!(replayed.random_blob, at_relay.random_blob, "the relay replays the blob it filed");
    assert_eq!(replayed.emitted, at_relay.emitted, "emitted is the original second");
    assert!(
        replayed.emitted < request_clock,
        "a replay is older than the request ({} < {request_clock}), not 'heard now'",
        replayed.emitted
    );
    assert_eq!(blob_emitted(&replayed.random_blob), replayed.emitted);
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(at_relay.emitted));

    // Idempotent replay: expire and ask again, the same emission comes back.
    assert!(b.expire_path(&hash).await);
    assert_eq!(b.latest_announce_emitted(&hash).await, None);
    b.request_path(&hash, None, None).await;
    let again = learn_announce("B's second path response", &mut b_announces, &hash, WAIT).await;
    assert!(again.is_path_response);
    assert_eq!(again.random_blob, at_relay.random_blob);
    assert_eq!(again.emitted, at_relay.emitted);
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(at_relay.emitted));

    // The same blob at equal hops and an equal second is not a newer path
    // (`RNS/Transport.py:2238`): asking once more without expiring surfaces no event.
    b.request_path(&hash, None, None).await;
    let quiet = timeout(QUIET, learn_announce("unexpected", &mut b_announces, &hash, WAIT)).await;
    let surfaced =
        quiet.ok().map(|event| (event.is_path_response, event.emitted, event.random_blob));
    assert_eq!(surfaced, None, "a known blob at an equal second is not re-announced");
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(at_relay.emitted));

    // A re-announces in a later second: B hears it unsolicited, strictly newer; every
    // accessor follows the max; and the relay now replays the newest announce.
    tokio::time::sleep_until((announced_at + PAST_REBROADCAST + NEXT_SECOND).into()).await;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    a.send_packet(packet).await;
    let second = learn_announce("A's re-announce at B", &mut b_announces, &hash, WAIT).await;
    assert!(!second.is_path_response, "the re-announce reaches B unsolicited via the relay");
    assert!(second.emitted > at_relay.emitted, "{} > {}", second.emitted, at_relay.emitted);
    assert_ne!(second.random_blob, at_relay.random_blob);
    assert_eq!(blob_emitted(&second.random_blob), second.emitted);
    let at_relay_2 =
        learn_announce("relay hears the re-announce", &mut relay.announces, &hash, WAIT).await;
    assert_eq!(at_relay_2.random_blob, second.random_blob, "relay and B see one emission");
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(second.emitted));
    assert_eq!(relay.transport.latest_announce_emitted(&hash).await, Some(second.emitted));

    assert!(b.expire_path(&hash).await);
    b.request_path(&hash, None, None).await;
    let newest =
        learn_announce("B's path response after re-announce", &mut b_announces, &hash, WAIT).await;
    assert!(newest.is_path_response);
    assert_eq!(newest.random_blob, second.random_blob, "the relay replays its newest announce");
    assert_eq!(newest.emitted, second.emitted);
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(second.emitted));

    timeout(WAIT, b.stop()).await.expect("b stop");
    timeout(WAIT, a.stop()).await.expect("a stop");
    timeout(WAIT, relay.transport.stop()).await.expect("relay stop");
}

/// The destination's own path response (it never announced) is a fresh emission each time
/// a request arrives in a later second: different blob, strictly newer second, and the
/// requester's accessor follows (parity with `RNS/Destination.py:308`). The requester is
/// connected directly to the node that owns the destination: a transport relay answers a
/// path request only from its own path table and does not forward an unknown one for a
/// plain (non-local-client) peer, in Python as here.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn own_path_responses_in_later_seconds_are_distinct_fresh_emissions() {
    // A plain (transport-off) listener owning the destination; B dials it directly.
    let a = Transport::new(TransportConfig::new(
        "announce-emitted-probe-own-a",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), a.iface_manager());
    let status = server.runtime_status_handle();
    a.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until_within("A to listen", WAIT, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    let (b, mut b_announces) = connector("announce-emitted-probe-own-b", port).await;
    wait_until_within("A to accept B", WAIT, || {
        status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let destination = a
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe", "announce-emitted-own"),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;
    assert_eq!(b.latest_announce_emitted(&hash).await, None);

    let before = clock_secs();
    let trace = b.request_path(&hash, None, None).await;
    assert!(trace.sent_ifaces >= 1, "B's path request left on an interface: {trace:?}");
    let first_at = Instant::now();
    let first = learn_announce("own response 1", &mut b_announces, &hash, WAIT).await;
    let after = clock_secs();
    assert!(first.is_path_response);
    assert!(
        before <= first.emitted && first.emitted <= after,
        "{} in {before}..={after}",
        first.emitted
    );
    assert_eq!(blob_emitted(&first.random_blob), first.emitted);
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(first.emitted));

    tokio::time::sleep_until((first_at + NEXT_SECOND).into()).await;
    assert!(b.expire_path(&hash).await);
    b.request_path(&hash, None, None).await;
    let second = learn_announce("own response 2", &mut b_announces, &hash, WAIT).await;
    assert!(second.is_path_response);
    assert!(second.emitted > first.emitted, "{} > {}", second.emitted, first.emitted);
    assert_ne!(second.random_blob, first.random_blob, "the destination emits afresh");
    assert_eq!(blob_emitted(&second.random_blob), second.emitted);
    assert_eq!(b.latest_announce_emitted(&hash).await, Some(second.emitted));

    timeout(WAIT, b.stop()).await.expect("b stop");
    timeout(WAIT, a.stop()).await.expect("a stop");
}
