//! Announce ingress admission, the usage patterns `announce_admission_usage_probe.rs` does
//! not reach, run from outside the crate over real TCP:
//!
//! - the exemption survives the shared instance going away and coming back: two clients
//!   attached with `SharedInstanceOptions::default()` whose shared instance is stopped and
//!   started again on its port reconnect, and one of them announcing THREE destinations
//!   back to back with no packet context is still learned in full by the other, with no
//!   `holding announce` record from any of the three transports (the client's interface and
//!   the restarted instance's fresh per-client child are both shared-instance interfaces,
//!   `RNS/Interfaces/LocalInterface.py:130`);
//! - the limiter still does its ordinary job under the three-sample floor: three announces
//!   1.5 s apart on a fresh, unflagged `TcpClient` interface measure well under the 3 Hz
//!   `burst_freq_new` (`RNS/Interfaces/Interface.py:76-84`) and are all learned, none held,
//!   so the floor admits the first two and the threshold, not the floor, decides the third.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport,
    TransportConfig,
};
use tokio::sync::{broadcast, mpsc};
use tokio::time::timeout;

#[path = "support/announce.rs"]
mod announce;
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
use free_loopback_port::free_loopback_port;
use hold_log::{holds_by, install_hold_logger};
use learn_all::learn_all;
use register_destinations::register_destinations;
use stop_all::stop_all;
use tcp_server_on::tcp_server_on;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// Three announces at this spacing read 15 Hz on an ordinary fresh interface, over the 3 Hz
/// `burst_freq_new` threshold.
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// Three announces at this spacing read about 1 Hz on an ordinary fresh interface, under
/// the 3 Hz `burst_freq_new` threshold.
const SPACED: Duration = Duration::from_millis(1500);
const APP_NAME: &str = "announce-admission-reconnect-probe";

const SHARED_INSTANCE: &str = "reconnect-probe-shared-instance";
const LEARNER: &str = "reconnect-probe-learner";
const ANNOUNCER: &str = "reconnect-probe-announcer";
const PLAIN_LISTENER: &str = "reconnect-probe-plain-listener";
const PLAIN_CONNECTOR: &str = "reconnect-probe-plain-connector";

/// A local client attached with `SharedInstanceOptions::default()` plus the state stream
/// and a short reconnect wait (neither touches admission), read through `Connected`.
struct Client {
    transport: Transport,
    events: mpsc::Receiver<SharedInstanceEvent>,
    announces: broadcast::Receiver<AnnounceEvent>,
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> SharedInstanceEvent {
    timeout(WAIT, events.recv())
        .await
        .unwrap_or_else(|_| panic!("{what}: the state stream timed out"))
        .unwrap_or_else(|| panic!("{what}: the state stream ended"))
}

/// Reads the state stream until `Connected`; a client redialing while the port is still
/// closed reports `DialFailed` and `Connecting` again first.
#[deny(clippy::await_holding_invalid_type)]
async fn await_connected(what: &str, events: &mut mpsc::Receiver<SharedInstanceEvent>) {
    let deadline = Instant::now() + WAIT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match timeout(remaining, events.recv()).await {
            Ok(Some(SharedInstanceEvent::Connected)) => return,
            Ok(Some(SharedInstanceEvent::Connecting | SharedInstanceEvent::DialFailed { .. })) => {}
            Ok(Some(other)) => panic!("{what}: unexpected {other:?} before `Connected`"),
            Ok(None) => panic!("{what}: the state stream ended before `Connected`"),
            Err(_) => panic!("{what}: `Connected` timed out"),
        }
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn local_client(name: &str, endpoint: SocketAddr) -> Client {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let (events_tx, mut events) = mpsc::channel(256);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(next_event(name, &mut events).await, SharedInstanceEvent::Connecting);
    assert_eq!(next_event(name, &mut events).await, SharedInstanceEvent::Connected);
    Client { transport, events, announces }
}

/// A plain, unflagged `TcpClient` connector to `endpoint`, its announce receiver subscribed
/// before the connect; returns once the stream is connected.
#[deny(clippy::await_holding_invalid_type)]
async fn plain_connector(
    name: &str,
    endpoint: SocketAddr,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(endpoint.to_string());
    let status = client.runtime_status_handle();
    let context = transport.iface_manager().lock().await.new_context(client);
    tokio::spawn(TcpClient::spawn(context));
    wait_until_within("the connector to connect", WAIT, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
}

/// Two default-option clients of a flagged shared instance; the instance is stopped and
/// started again on its port, both clients redial it, and only then does one of them
/// announce three destinations 100 ms apart with no packet context. The other learns all
/// three within 5 s and no transport logs a `holding announce` record: the reconnected
/// client's interface and the restarted instance's fresh per-client child are both
/// shared-instance interfaces and never ingress-limit.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn reconnected_local_clients_still_admit_three_back_to_back_announces() {
    install_hold_logger();
    let names = [SHARED_INSTANCE, LEARNER, ANNOUNCER];
    let port = free_loopback_port().await;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let (shared, _) = tcp_server_on(SHARED_INSTANCE, port, true).await;
    let mut learner = local_client(LEARNER, endpoint).await;
    let mut announcer = local_client(ANNOUNCER, endpoint).await;

    // The shared instance goes away and comes back on its port.
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
    for (name, client) in [(LEARNER, &mut learner), (ANNOUNCER, &mut announcer)] {
        assert_eq!(
            next_event(name, &mut client.events).await,
            SharedInstanceEvent::Disconnected,
            "{name}: the drop is served"
        );
        assert_eq!(
            next_event(name, &mut client.events).await,
            SharedInstanceEvent::Connecting,
            "{name}: `Connecting` follows the drop"
        );
    }
    let (restarted, _) = tcp_server_on(SHARED_INSTANCE, port, true).await;
    for (name, client) in [(LEARNER, &mut learner), (ANNOUNCER, &mut announcer)] {
        await_connected(
            &format!("{name}: the redial on the restarted instance"),
            &mut client.events,
        )
        .await;
    }
    assert!(
        learner.transport.connected_to_shared_instance().await,
        "the learner holds the client posture after the reconnect"
    );
    // A broadcast subscription only sees later events.
    learner.announces = learner.transport.recv_announces().await;

    let destinations = register_destinations(&announcer.transport, APP_NAME, "d", 3).await;
    for (destination, _hash) in &destinations {
        announce(&announcer.transport, destination).await;
        tokio::time::sleep(BACK_TO_BACK).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut learner.announces, &hashes, WAIT).await;
    assert!(
        missing.is_empty(),
        "after the reconnect the learner did not learn {} of the three destinations within \
         {WAIT:?} ({:?}); hold records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "all three were learned only after {:?}; a held announce would surface no sooner than \
         30 s",
        started.elapsed()
    );
    assert!(
        holds_by(&names).is_empty(),
        "a shared-instance interface held an announce after the reconnect: {:?}",
        holds_by(&names)
    );

    drop(destinations);
    stop_all(vec![announcer.transport, learner.transport, restarted]).await;
}

/// Three announces 1.5 s apart on a fresh, unflagged `TcpClient` interface are all learned
/// and none is held: the measured frequency is about 1 Hz, under the 3 Hz `burst_freq_new`
/// (`RNS/Interfaces/Interface.py:76-84`), so the limiter's threshold, not the three-sample
/// floor, decides the third.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn ordinary_fresh_interface_admits_three_announces_spaced_over_a_second() {
    install_hold_logger();
    let names = [PLAIN_LISTENER, PLAIN_CONNECTOR];
    let port = free_loopback_port().await;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let (listener, _) = tcp_server_on(PLAIN_LISTENER, port, false).await;
    let (connector, mut announces) = plain_connector(PLAIN_CONNECTOR, endpoint).await;
    let destinations = register_destinations(&listener, APP_NAME, "d", 3).await;

    for (idx, (destination, _hash)) in destinations.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(SPACED).await;
        }
        announce(&listener, destination).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut announces, &hashes, WAIT).await;
    assert!(
        missing.is_empty(),
        "the connector did not learn {} of three announces 1.5 s apart within {WAIT:?} ({:?}); \
         hold records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the three spaced announces were learned only after {:?}",
        started.elapsed()
    );
    assert!(
        holds_by(&names).is_empty(),
        "an announce under the threshold was held: {:?}",
        holds_by(&names)
    );
    for (idx, hash) in hashes.iter().enumerate() {
        assert!(connector.destination_desc(hash).await.is_some(), "the connector resolves d{idx}");
    }

    drop(destinations);
    stop_all(vec![connector, listener]).await;
}
