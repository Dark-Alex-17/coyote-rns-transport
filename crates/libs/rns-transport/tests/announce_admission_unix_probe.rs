//! Announce ingress admission over a Unix-domain shared instance, from outside the crate.
//! Every other admission pin (`announce_admission_usage_probe.rs`,
//! `announce_admission_mixed_iface_probe.rs`, `announce_admission_reconnect_probe.rs`,
//! `shared_instance_one_identity_two_clients_probe.rs` and the reticulumd probe) runs over TCP;
//! this file checks that the exemption follows the shared-instance FLAG, not the interface
//! kind:
//!
//! - a `LocalUnixServer` flagged as the shared instance and its default-option clients
//!   (`SharedInstanceEndpoint::Unix`, `SharedInstanceOptions::default()`) never ingress-limit
//!   (`RNS/Interfaces/LocalInterface.py:130`): one client announces FOUR destinations back to
//!   back and a second client one more 100 ms later; a third client learns all five inside
//!   5 s and no transport logs a `holding announce` record;
//! - an UNFLAGGED `LocalUnixServer` is an ordinary interface: a plain `LocalUnixClient`
//!   announcing three destinations 100 ms apart has the first two learned by the listener and
//!   the third held (`Interface.py:84`, `:346-348`: 0 Hz until more than two samples; three at
//!   100 ms read 15 Hz, over the 3 Hz `burst_freq_new` of `AnnounceRateLimit::default()`),
//!   logged once as `holding announce ... before queue` by the listener's transport.

#![cfg(unix)]

use std::path::Path;
use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint, LocalUnixServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;

#[path = "support/announce.rs"]
mod announce;
#[path = "support/hold_log.rs"]
mod hold_log;
#[path = "support/learn_all.rs"]
mod learn_all;
#[path = "support/register_destinations.rs"]
mod register_destinations;
#[path = "support/stop_all.rs"]
mod stop_all;
#[path = "support/wait_until_within.rs"]
mod wait_until_within;
use announce::announce;
use hold_log::{holds_by, install_hold_logger};
use learn_all::learn_all;
use register_destinations::register_destinations;
use stop_all::stop_all;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
/// Three announces at this spacing read 15 Hz on an ordinary fresh interface.
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// The window in which a held announce must NOT surface (the hold alone is 15 s).
const HELD: Duration = Duration::from_secs(3);
const APP_NAME: &str = "announce-admission-unix-probe";

const FLAGGED_SERVER: &str = "unix-probe-shared-instance";
const FLAGGED_LEARNER: &str = "unix-probe-learner";
const FLAGGED_BURST: &str = "unix-probe-burst";
const FLAGGED_SINGLE: &str = "unix-probe-single";
const UNFLAGGED_SERVER: &str = "unix-probe-plain-listener";
const UNFLAGGED_CONNECTOR: &str = "unix-probe-plain-connector";

/// A transport-enabled node with a `LocalUnixServer` on `socket`, flagged as the shared
/// instance when asked and left an ordinary listener otherwise; its announce receiver is
/// subscribed before the spawn. Transport mode stays enabled for the flagged and the
/// unflagged listener alike: the admission decision never reads it, so the flag is the
/// only variable between the two tests.
#[deny(clippy::await_holding_invalid_type)]
async fn unix_server(
    name: &str,
    socket: &Path,
    shared_instance: bool,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let mut config = TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false);
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let announces = transport.recv_announces().await;
    let server = LocalUnixServer::new_endpoint(
        LocalUnixEndpoint::filesystem(socket.to_path_buf()),
        transport.iface_manager(),
    );
    let listener = transport.iface_manager().lock().await.spawn(server, LocalUnixServer::spawn);
    if shared_instance {
        assert!(
            transport.iface_manager().lock().await.set_shared_instance(listener, true),
            "the listener is flagged as the shared instance"
        );
    }
    wait_until_within("the unix socket to be bound", WAIT, || socket.exists()).await;
    (transport, announces)
}

/// A local client attached over `SharedInstanceEndpoint::Unix` with
/// `SharedInstanceOptions::default()`; its announce receiver is subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn unix_local_client(
    name: &str,
    socket: &Path,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.to_path_buf())),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the unix attach is accepted");
    (transport, announces)
}

/// A plain, unflagged `LocalUnixClient` connector to `socket`; returns once its first
/// connect event fired.
#[deny(clippy::await_holding_invalid_type)]
async fn plain_unix_connector(name: &str, socket: &Path) -> Transport {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(socket.to_path_buf()))
        .with_connect_events(connect_tx);
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(LocalUnixClient::spawn(context));
    timeout(WAIT, connect_rx.recv())
        .await
        .expect("the plain unix connector connects within WAIT")
        .expect("connect event sender alive");
    transport
}

/// A flagged Unix-domain shared instance never ingress-limits its clients: four back-to-back
/// announces from one default-option client plus one from another 100 ms later all reach a
/// third client inside 5 s, through the server's two per-client children and the learner's
/// own shared-instance interface, with no `holding announce` record from any of the four
/// transports.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn unix_shared_instance_clients_announcing_back_to_back_are_learned_in_full() {
    install_hold_logger();
    let names = [FLAGGED_SERVER, FLAGGED_LEARNER, FLAGGED_BURST, FLAGGED_SINGLE];
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("si.sock");
    let (shared, _shared_announces) = unix_server(FLAGGED_SERVER, &socket, true).await;
    let (learner, mut learner_announces) = unix_local_client(FLAGGED_LEARNER, &socket).await;
    let (burst, _burst_announces) = unix_local_client(FLAGGED_BURST, &socket).await;
    let (single, _single_announces) = unix_local_client(FLAGGED_SINGLE, &socket).await;

    let burst_destinations = register_destinations(&burst, APP_NAME, "d", 4).await;
    let single_destinations = register_destinations(&single, APP_NAME, "d", 1).await;
    for (destination, _hash) in &burst_destinations {
        announce(&burst, destination).await;
    }
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&single, &single_destinations[0].0).await;

    let hashes: Vec<AddressHash> = burst_destinations
        .iter()
        .chain(single_destinations.iter())
        .map(|(_, hash)| *hash)
        .collect();
    let started = Instant::now();
    let missing = learn_all(&mut learner_announces, &hashes, WAIT).await;
    assert!(
        missing.is_empty(),
        "the learner did not learn {} of the five destinations within {WAIT:?} ({:?}); hold \
         records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "all five were learned only after {:?}; a held announce would surface no sooner than \
         30 s",
        started.elapsed()
    );
    for hash in &hashes {
        assert!(
            learner.destination_desc(hash).await.is_some(),
            "the learner resolves {hash} after the burst"
        );
    }
    assert!(
        holds_by(&names).is_empty(),
        "a shared-instance interface held an announce over unix: {:?}",
        holds_by(&names)
    );

    drop(burst_destinations);
    drop(single_destinations);
    stop_all(vec![single, burst, learner, shared]).await;
}

/// Positive control: the same `LocalUnixServer` left UNFLAGGED is an ordinary interface. A
/// plain `LocalUnixClient` announcing three destinations 100 ms apart has the first two
/// learned by the listener's transport without spacing (0 Hz until more than two samples)
/// and the third held for hold plus penalty, logged once as `holding announce` by the
/// listener's transport and by nobody else.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn unflagged_unix_listener_holds_the_third_of_three_announces() {
    install_hold_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("plain.sock");
    let (listener, mut listener_announces) = unix_server(UNFLAGGED_SERVER, &socket, false).await;
    let connector = plain_unix_connector(UNFLAGGED_CONNECTOR, &socket).await;

    let destinations = register_destinations(&connector, APP_NAME, "d", 3).await;
    for (idx, (destination, _hash)) in destinations.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(BACK_TO_BACK).await;
        }
        announce(&connector, destination).await;
    }
    let first_two: Vec<AddressHash> = destinations[..2].iter().map(|(_, hash)| *hash).collect();
    let third = destinations[2].1;

    let missing = learn_all(&mut listener_announces, &first_two, WAIT).await;
    assert!(
        missing.is_empty(),
        "the listener did not learn the first two announces ({missing:?}); hold records: {:?}",
        holds_by(&[UNFLAGGED_SERVER, UNFLAGGED_CONNECTOR])
    );
    let third_missing = learn_all(&mut listener_announces, &[third], HELD).await;
    assert_eq!(
        third_missing,
        vec![third],
        "the third announce at 15 Hz on an ordinary interface must be held past {HELD:?}"
    );
    let holds = holds_by(&[UNFLAGGED_SERVER]);
    assert_eq!(
        holds.len(),
        1,
        "the unflagged listener holds exactly the third announce: {holds:?}"
    );
    assert!(
        holds[0].contains(&format!("holding announce for {third}")),
        "the hold record names the third destination: {holds:?}"
    );
    assert!(
        holds_by(&[UNFLAGGED_CONNECTOR]).is_empty(),
        "the connector received no announce to hold: {:?}",
        holds_by(&[UNFLAGGED_CONNECTOR])
    );

    drop(destinations);
    stop_all(vec![connector, listener]).await;
}
