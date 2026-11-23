//! Second usage probe for `Transport::path_interface`, the patterns the first probe and
//! the API twin leave open, from outside the crate:
//!
//! - a path learned THROUGH a transport-enabled relay: the record on the far side reads
//!   `hops == 2 == hops_to`, `iface` is the far side's own client interface toward the
//!   relay (not anything of the announcer's), and the relay's own record reads `hops == 1`
//!   on the interface `path_status` names there;
//! - the record agrees field by field with the `Transport::path_table` row for the same
//!   destination: `iface == interface`, `hops == hops`, `name == interface_name`, and
//!   `learned_at` is the row's `timestamp_secs` read through the other clock sample;
//! - two destinations on one interface each have their own record, and expiring one
//!   leaves the other's record untouched;
//! - the read is `Send` (usable from a spawned task), and it is a pure read: a thousand
//!   reads of one entry return the same `iface`, `hops` and `name`;
//! - `PathIface` is `#[non_exhaustive]`: a struct literal and a pattern without `..` do
//!   not compile outside the crate, while reading by field does (driven through `rustc`
//!   against this binary's rlib, the way `breaking_changes_compile_fail.rs` does).
//!
//! Three transports on 127.0.0.1: a transport-enabled `TcpServer` relay that rebroadcasts
//! each announce once, and two `TcpClient` connectors A (announcer) and B (learner).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, PathIface, Transport, TransportConfig};
use tokio::sync::broadcast;
use tokio::time::timeout;

#[path = "support/compile_fail.rs"]
mod compile_fail;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/learn_announce.rs"]
mod learn_announce;
#[path = "support/wait_until_within.rs"]
mod wait_until_within;
use compile_fail::{artifact_root, candidate_rlibs, rustc, Scratch};
use free_loopback_port::free_loopback_port;
use learn_announce::learn_announce;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
const SECOND: Duration = Duration::from_secs(1);
/// Two conversions of one `Instant` through two wall-clock samples agree to the sampling
/// imprecision; a relearn or a wrong source would move the figure by far more.
const CLOCK_SLACK: Duration = Duration::from_millis(50);

struct Relay {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
}

/// A transport-enabled listener that rebroadcasts each announce exactly once.
#[deny(clippy::await_holding_invalid_type)]
async fn relay(name: &str) -> Relay {
    let mut config = TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false);
    config.set_transport_enabled(true);
    config.set_announce_retry_limit(0);
    let transport = Transport::new(config);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until_within("the relay to listen", WAIT, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    Relay { transport, port, status }
}

/// A `TcpClient` connector spawned through the manager, so the test holds the address the
/// manager minted for the client; the announce receiver exists before the stream is up.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(
    name: &str,
    port: u16,
) -> (Transport, AddressHash, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    wait_until_within(&format!("{name} to connect"), WAIT, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, iface, announces)
}

#[deny(clippy::await_holding_invalid_type)]
async fn await_accepted(relay: &Relay, count: u64) {
    wait_until_within(&format!("the relay to accept {count}"), WAIT, || {
        relay.status.to_json()["accepted_connections"].as_u64() == Some(count)
    })
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn announce_from(transport: &Transport, aspect: &str) -> AddressHash {
    let destination = transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("probe2", aspect),
        )
        .await;
    let hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
    hash
}

fn within_bracket(at: SystemTime, before: SystemTime, after: SystemTime) -> bool {
    before - SECOND <= at && at <= after + SECOND
}

fn abs_diff(a: SystemTime, b: SystemTime) -> Duration {
    a.duration_since(b).unwrap_or_else(|earlier| earlier.duration())
}

fn assert_send<F: Send>(_: &F) {}

/// A path learned through a relay: the far side's record reads two hops on its own
/// interface toward the relay, agreeing with `hops_to` and `path_status`; the relay's own
/// record reads one hop on the interface `path_status` names there.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_path_through_a_relay_reads_two_hops_on_the_learners_own_interface() {
    let relay = relay("path-interface-relay").await;
    let (a, _a_iface, _a_announces) = connector("path-interface-relay-a", relay.port).await;
    await_accepted(&relay, 1).await;
    let (b, b_iface, mut b_announces) = connector("path-interface-relay-b", relay.port).await;
    await_accepted(&relay, 2).await;
    assert!(b.iface_manager().lock().await.set_display_name(b_iface, Some("to-relay".to_owned())));

    let before = SystemTime::now();
    let hash = announce_from(&a, "two-hops").await;
    learn_announce("B hears A through the relay", &mut b_announces, &hash, WAIT).await;
    let after = SystemTime::now();

    let at_b: PathIface = b.path_interface(&hash).await.expect("B has the path on file");
    assert_eq!(at_b.hops, 2, "one relay between A and B");
    assert_eq!(at_b.hops, b.hops_to(&hash).await, "hops is the figure hops_to answers");
    // Interface addresses are minted per manager from a counter, so the tag is only
    // meaningful inside its own transport; the record is judged against B's own tag,
    // which is all a consumer joining on `iface` ever holds.
    assert_eq!(at_b.iface, b_iface, "B's own client interface toward the relay");
    assert_eq!(b.path_status(&hash).await.interface, Some(at_b.iface), "path_status agrees");
    assert_eq!(at_b.name.as_deref(), Some("to-relay"), "B's display name for its client");
    assert!(
        within_bracket(at_b.learned_at, before, after),
        "learned_at {:?} within a second of {before:?}..={after:?}",
        at_b.learned_at
    );

    let at_relay = relay.transport.path_interface(&hash).await.expect("the relay filed the path");
    assert_eq!(at_relay.hops, 1, "A is the relay's direct peer");
    assert_eq!(at_relay.hops, relay.transport.hops_to(&hash).await);
    let relay_status = relay.transport.path_status(&hash).await;
    assert_eq!(Some(at_relay.iface), relay_status.interface, "the accepted child A arrived on");
    assert_ne!(at_relay.iface, at_b.iface, "each transport names its own interface");
    assert!(
        within_bracket(at_relay.learned_at, before, after),
        "the relay's learned_at {:?} within a second of {before:?}..={after:?}",
        at_relay.learned_at
    );
    assert!(
        at_relay.learned_at <= at_b.learned_at + CLOCK_SLACK,
        "the relay learned the path no later than B: relay {:?}, B {:?}",
        at_relay.learned_at,
        at_b.learned_at
    );

    // A never hears its own announce back: no path table entry for its own destination.
    assert_eq!(a.path_interface(&hash).await, None, "the announcer holds no row for itself");

    for transport in [a, b, relay.transport] {
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
}

/// The record and the `path_table` row for one destination are two reads of one entry:
/// the same interface, hops and display name, and `learned_at` is `timestamp_secs` through
/// the other clock sample. Two destinations on one interface keep separate records, and
/// expiring one leaves the other's untouched.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_record_agrees_with_the_path_table_row_and_entries_are_independent() {
    let relay = relay("path-interface-rows-relay").await;
    let (a, _a_iface, _a_announces) = connector("path-interface-rows-a", relay.port).await;
    await_accepted(&relay, 1).await;
    let (b, b_iface, mut b_announces) = connector("path-interface-rows-b", relay.port).await;
    await_accepted(&relay, 2).await;
    assert!(b.iface_manager().lock().await.set_display_name(b_iface, Some("uplink".to_owned())));

    let first = announce_from(&a, "first").await;
    learn_announce("B hears the first", &mut b_announces, &first, WAIT).await;
    // Python keys the emission on whole seconds; a second destination is a new blob anyway,
    // but crossing the second keeps the two learn times apart for the check below.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let second = announce_from(&a, "second").await;
    learn_announce("B hears the second", &mut b_announces, &second, WAIT).await;

    let rows = b.path_table(None).await.expect("path table");
    for hash in [first, second] {
        let record = b.path_interface(&hash).await.expect("the path is on file");
        let row = rows.iter().find(|row| row.destination == hash).expect("the table lists it");
        assert_eq!(record.iface, row.interface, "the same interface");
        assert_eq!(record.hops, row.hops, "the same hops");
        assert_eq!(record.name, row.interface_name, "the same display name");
        assert_eq!(record.name.as_deref(), Some("uplink"));
        let row_at = UNIX_EPOCH + Duration::from_secs_f64(row.timestamp_secs);
        assert!(
            abs_diff(record.learned_at, row_at) < CLOCK_SLACK,
            "learned_at {:?} is the row's timestamp_secs {row_at:?} through the other clock sample",
            record.learned_at
        );
    }
    let first_record = b.path_interface(&first).await.expect("on file");
    let second_record = b.path_interface(&second).await.expect("on file");
    assert_ne!(first_record, second_record, "two destinations, two records");
    assert!(
        second_record.learned_at > first_record.learned_at + Duration::from_millis(500),
        "each record carries its own learn time"
    );

    // A thousand reads are one read: no mutation, no drift beyond clock sampling.
    let future = b.path_interface(&first);
    assert_send(&future);
    let reference = future.await.expect("on file");
    for _ in 0..1000 {
        let again = b.path_interface(&first).await.expect("still on file");
        assert_eq!(again.iface, reference.iface);
        assert_eq!(again.hops, reference.hops);
        assert_eq!(again.name, reference.name);
        assert!(abs_diff(again.learned_at, reference.learned_at) < CLOCK_SLACK);
    }
    // The read is Send: it runs on a spawned task that owns the transport for a moment.
    let (from_task, b) = tokio::spawn(async move {
        let record = b.path_interface(&first).await;
        (record, b)
    })
    .await
    .expect("task joins");
    assert_eq!(
        from_task.map(|r| (r.iface, r.hops, r.name)),
        Some((reference.iface, reference.hops, reference.name.clone()))
    );

    // Expiring one leaves the other alone.
    assert!(b.expire_path(&first).await);
    assert_eq!(b.path_interface(&first).await, None);
    let kept = b.path_interface(&second).await.expect("the second record survives");
    assert_eq!(kept.iface, second_record.iface);
    assert_eq!(kept.hops, second_record.hops);
    assert_eq!(kept.name, second_record.name);
    assert!(abs_diff(kept.learned_at, second_record.learned_at) < CLOCK_SLACK);

    for transport in [a, b, relay.transport] {
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
}

/// Reading `PathIface` by field compiles for a consumer; this proves the harness works
/// against the rlib before the rejected snippets below are judged.
const FIELD_READER: &str = r#"
use std::time::SystemTime;
use rns_transport::hash::AddressHash;
use rns_transport::transport::PathIface;

pub fn fields(path: &PathIface) -> (AddressHash, Option<&str>, SystemTime, u8) {
    (path.iface, path.name.as_deref(), path.learned_at, path.hops)
}

pub fn with_rest(path: &PathIface) -> u8 {
    let PathIface { hops, .. } = path;
    *hops
}
"#;

/// The shapes `#[non_exhaustive]` forbids outside the crate, with the diagnostic a
/// consumer sees.
const REJECTED: &[(&str, &str, &str)] = &[
    (
        "path_iface_struct_literal",
        "use std::time::SystemTime;\n\
         use rns_transport::hash::AddressHash;\n\
         use rns_transport::transport::PathIface;\n\
         pub fn build(iface: AddressHash) -> PathIface {\n\
             PathIface { iface, name: None, learned_at: SystemTime::UNIX_EPOCH, hops: 1 }\n\
         }\n",
        "cannot create non-exhaustive struct using struct expression",
    ),
    (
        "path_iface_exhaustive_pattern",
        "use rns_transport::transport::PathIface;\n\
         pub fn hops(path: &PathIface) -> u8 {\n\
             let PathIface { iface: _, name: _, learned_at: _, hops } = path;\n\
             *hops\n\
         }\n",
        "`..` required with struct marked as non-exhaustive",
    ),
];

/// `PathIface` is `#[non_exhaustive]`: a field may be added in a patch release without
/// breaking a consumer who reads by field.
#[test]
fn path_iface_is_non_exhaustive_for_a_consumer() {
    let scratch = Scratch::new("path-iface-non-exhaustive");
    let deps = artifact_root();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib under {}", deps.display());
    let mut failures = Vec::new();
    let mut rlib = None;
    for candidate in candidates {
        let output = rustc(&scratch.0, &deps, &candidate, "field_reader", FIELD_READER);
        if output.status.success() {
            rlib = Some(candidate);
            break;
        }
        failures.push(format!(
            "{}:\n{}",
            candidate.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let rlib = rlib.unwrap_or_else(|| {
        panic!(
            "reading PathIface by field should compile against the crate's rlib:\n{}",
            failures.join("\n")
        )
    });
    let mut unexpected = Vec::new();
    for (name, source, promised) in REJECTED {
        let output = rustc(&scratch.0, &deps, &rlib, name, source);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            unexpected
                .push(format!("{name}: compiled, but PathIface is documented #[non_exhaustive]"));
        } else if !stderr.contains(promised) {
            unexpected.push(format!("{name}: failed, but not with {promised:?}:\n{stderr}"));
        }
    }
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n\n"));
}
