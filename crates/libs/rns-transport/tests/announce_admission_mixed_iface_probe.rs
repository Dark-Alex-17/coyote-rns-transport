//! The shared-instance announce exemption is scoped to the interface, not the transport,
//! exercised from outside the crate over real TCP: ONE transport is at once a local client
//! of a flagged `TcpServer` (attached with `SharedInstanceOptions::default()`) and a plain,
//! unflagged `TcpClient` connector of an ordinary listener.
//!
//! - Three announces back to back from the ordinary listener reach the transport on its
//!   unflagged interface: the first two are learned (the measurement reads 0 Hz until more
//!   than `IC_DEQUE_MIN_SAMPLE` = 2 samples exist, `RNS/Interfaces/Interface.py:84`,
//!   `:346-348`), the third is held and the transport logs `holding announce` for it.
//! - Three announces back to back from another local client of the shared instance reach the
//!   same transport on its shared-instance interface right after that hold: all three are
//!   learned inside 5 s and none is held (`RNS/Interfaces/LocalInterface.py:130`), so the
//!   per-interface flag is read for the interface the announce arrived on, and a hold on the
//!   ordinary interface does not spill over onto the shared-instance one.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};

#[path = "support/announce.rs"]
mod announce;
#[path = "support/announce_within.rs"]
mod announce_within;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/hold_log.rs"]
mod hold_log;
#[path = "support/learn_all.rs"]
mod learn_all;
#[path = "support/register_destinations.rs"]
mod register_destinations;
#[path = "support/stop_all.rs"]
mod stop_all;
#[path = "support/tcp_server_on.rs"]
mod tcp_server_on;
#[path = "support/wait_until_within.rs"]
mod wait_until_within;
use announce::announce;
use announce_within::announce_within;
use free_loopback_port::free_loopback_port;
use hold_log::{holds_by, install_hold_logger};
use learn_all::learn_all;
use register_destinations::register_destinations;
use stop_all::stop_all;
use tcp_server_on::tcp_server_on;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// The window in which a held announce must NOT surface (the hold alone is 15 s).
const HELD: Duration = Duration::from_secs(3);
/// The window in which an exempt burst MUST be learned in full.
const PROMPT: Duration = Duration::from_secs(5);
const APP_NAME: &str = "announce-admission-mixed-probe";
const MIXED: &str = "probe-mixed-client";

/// The hold records logged by transport `name` for destination `hash`.
fn holds_for(name: &str, hash: &AddressHash) -> Vec<String> {
    let dst = format!("{hash}");
    holds_by(&[name]).into_iter().filter(|message| message.contains(&dst)).collect()
}

/// Attaches `transport` to the shared instance with the default options.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(transport: &Transport, endpoint: SocketAddr) {
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the attach is accepted");
}

/// Adds a plain, unflagged `TcpClient` interface to `endpoint` on `transport` and waits for
/// the connect.
#[deny(clippy::await_holding_invalid_type)]
async fn connect_plain(transport: &Transport, endpoint: SocketAddr) {
    let client = TcpClient::new(endpoint.to_string());
    let status = client.runtime_status_handle();
    let context = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        manager.new_context(client)
    };
    tokio::spawn(TcpClient::spawn(context));
    wait_until_within("the plain connector to connect", WAIT, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_transport_limits_its_plain_interface_and_exempts_its_shared_instance_interface() {
    install_hold_logger();
    let shared_port = free_loopback_port().await;
    let shared_endpoint = SocketAddr::from(([127, 0, 0, 1], shared_port));
    let (shared, _) = tcp_server_on("probe-mixed-shared-instance", shared_port, true).await;
    let plain_port = free_loopback_port().await;
    let plain_endpoint = SocketAddr::from(([127, 0, 0, 1], plain_port));
    let (plain_listener, _) = tcp_server_on("probe-mixed-plain-listener", plain_port, false).await;

    let mixed =
        Transport::new(TransportConfig::new(MIXED, &PrivateIdentity::new_from_rand(OsRng), false));
    let mut mixed_announces = mixed.recv_announces().await;
    attach(&mixed, shared_endpoint).await;
    connect_plain(&mixed, plain_endpoint).await;

    let announcer = Transport::new(TransportConfig::new(
        "probe-mixed-announcer",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    attach(&announcer, shared_endpoint).await;

    // The ordinary interface: three back to back, the third held.
    let plain = register_destinations(&plain_listener, APP_NAME, "p", 3).await;
    announce(&plain_listener, &plain[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&plain_listener, &plain[1].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&plain_listener, &plain[2].0).await;

    let missing = learn_all(&mut mixed_announces, &[plain[0].1, plain[1].1], PROMPT).await;
    assert!(missing.is_empty(), "the first two plain announces were not learned: {missing:?}");
    let early = announce_within(&mut mixed_announces, &plain[2].1, HELD).await;
    assert!(
        early.is_err(),
        "the third plain announce inside one second was admitted after {:?}; the ordinary \
         interface must hold it",
        early.unwrap_or_default()
    );
    let plain_holds = holds_for(MIXED, &plain[2].1);
    assert_eq!(
        plain_holds.len(),
        1,
        "exactly one hold record for the third plain announce on the mixed transport: {plain_holds:?}"
    );
    eprintln!("ordinary interface: {}", plain_holds[0]);

    // The shared-instance interface of the SAME transport, right after that hold: three back
    // to back, all learned, none held.
    let burst = register_destinations(&announcer, APP_NAME, "s", 3).await;
    for (destination, _hash) in &burst {
        announce(&announcer, destination).await;
    }
    let hashes: Vec<AddressHash> = burst.iter().map(|(_, hash)| *hash).collect();
    let started = Instant::now();
    let missing = learn_all(&mut mixed_announces, &hashes, PROMPT).await;
    assert!(
        missing.is_empty(),
        "the mixed transport did not learn {} of three back-to-back announces on its \
         shared-instance interface within {PROMPT:?} ({missing:?}); its hold records: {:?}",
        missing.len(),
        hashes.iter().flat_map(|hash| holds_for(MIXED, hash)).collect::<Vec<_>>()
    );
    eprintln!("shared-instance interface: all three learned after {:?}", started.elapsed());
    for hash in &hashes {
        assert!(
            holds_for(MIXED, hash).is_empty(),
            "the shared-instance interface held {hash}: {:?}",
            holds_for(MIXED, hash)
        );
        assert!(
            mixed.destination_desc(hash).await.is_some(),
            "the mixed transport resolves {hash}"
        );
    }
    // The ordinary interface's hold is still the only one on the transport.
    assert_eq!(
        holds_by(&[MIXED]).len(),
        1,
        "the mixed transport logged a hold beyond the third plain announce"
    );

    drop(plain);
    drop(burst);
    stop_all(vec![announcer, mixed, plain_listener, shared]).await;
}
