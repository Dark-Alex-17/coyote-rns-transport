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
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport,
    TransportConfig,
};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
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

/// Captures the limiter's `tp(<name>): holding announce for <dst> before queue for <delay>`
/// records; `log::set_logger` is process-wide, so the records are attributed to a test by
/// the transport name in the message.
struct HoldLogger {
    records: std::sync::Mutex<Vec<String>>,
}

static HOLDS: HoldLogger = HoldLogger { records: std::sync::Mutex::new(Vec::new()) };

impl log::Log for HoldLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::transport")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let message = record.args().to_string();
        if message.contains("holding announce") {
            self.records.lock().expect("hold record store").push(message);
        }
    }

    fn flush(&self) {}
}

fn install_hold_logger() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&HOLDS).expect("no other logger in this test binary");
        log::set_max_level(log::LevelFilter::Debug);
    });
}

/// The hold records logged by the transports named in `names`.
fn holds_by(names: &[&str]) -> Vec<String> {
    HOLDS
        .records
        .lock()
        .expect("hold record store")
        .iter()
        .filter(|message| names.iter().any(|name| message.contains(&format!("tp({name})"))))
        .cloned()
        .collect()
}

#[deny(clippy::await_holding_invalid_type)]
async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(POLL).await;
    }
}

/// Waits up to `WAIT` for the announces of every hash in `expected`, in whatever order they
/// arrive; returns the hashes still missing when the window passed.
#[deny(clippy::await_holding_invalid_type)]
async fn learn_all(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    expected: &[AddressHash],
) -> Vec<AddressHash> {
    let mut missing: Vec<AddressHash> = expected.to_vec();
    let deadline = Instant::now() + WAIT;
    while !missing.is_empty() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, announces.recv()).await {
            Ok(Ok(event)) => {
                let address_hash = event.destination.lock().await.desc.address_hash;
                if let Some(idx) = missing.iter().position(|hash| *hash == address_hash) {
                    eprintln!("learned {address_hash}");
                    missing.swap_remove(idx);
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_elapsed) => break,
        }
    }
    missing
}

/// A node with a `TcpServer` on `port`, flagged as the shared instance when asked (rnsd's
/// topology, which also needs `transport_enabled`) and an ordinary listener otherwise.
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_server_on(name: &str, port: u16, shared_instance: bool) -> Transport {
    let mut config = TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false);
    config.set_transport_enabled(shared_instance);
    let transport = Transport::new(config);
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    let listener = transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    if shared_instance {
        assert!(
            transport.iface_manager().lock().await.set_shared_instance(listener, true),
            "the listener is flagged as the shared instance"
        );
    }
    wait_until("the listener to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    transport
}

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

/// Registers `count` destinations, each under a fresh identity, on `transport`.
#[deny(clippy::await_holding_invalid_type)]
async fn register_destinations(
    transport: &Transport,
    count: usize,
) -> Vec<(Arc<Mutex<SingleInputDestination>>, AddressHash)> {
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let destination = transport
            .add_destination(
                PrivateIdentity::new_from_rand(OsRng),
                DestinationName::new(APP_NAME, &format!("d{idx}")),
            )
            .await;
        let hash = destination.lock().await.desc.address_hash;
        out.push((destination, hash));
    }
    out
}

/// Announces `destination` with no packet context, so the receiving limiter weighs it.
#[deny(clippy::await_holding_invalid_type)]
async fn announce(transport: &Transport, destination: &Arc<Mutex<SingleInputDestination>>) {
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
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
    wait_until("the connector to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
}

#[deny(clippy::await_holding_invalid_type)]
async fn stop_all(transports: Vec<Transport>) {
    for transport in transports {
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
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
    let shared = tcp_server_on(SHARED_INSTANCE, port, true).await;
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
    let restarted = tcp_server_on(SHARED_INSTANCE, port, true).await;
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

    let destinations = register_destinations(&announcer.transport, 3).await;
    for (destination, _hash) in &destinations {
        announce(&announcer.transport, destination).await;
        tokio::time::sleep(BACK_TO_BACK).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut learner.announces, &hashes).await;
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
    let listener = tcp_server_on(PLAIN_LISTENER, port, false).await;
    let (connector, mut announces) = plain_connector(PLAIN_CONNECTOR, endpoint).await;
    let destinations = register_destinations(&listener, 3).await;

    for (idx, (destination, _hash)) in destinations.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(SPACED).await;
        }
        announce(&listener, destination).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut announces, &hashes).await;
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
