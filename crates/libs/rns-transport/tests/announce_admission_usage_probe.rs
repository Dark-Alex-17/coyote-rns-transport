//! Announce ingress admission exercised from outside the crate over real TCP:
//!
//! - a shared-instance interface never ingress-limits, on the client's side AND on the child a
//!   shared instance holds per accepted client (`RNS/Interfaces/LocalInterface.py:130`): one
//!   local client announcing THREE destinations back to back is learned in full by another
//!   local client, so neither the server's per-client child nor the learner's own interface
//!   holds the second or third (`third_client_admits_two_local_client_announces_back_to_back`
//!   in `shared_instance_one_identity_two_clients_probe.rs` spreads two announces over two
//!   children; this one puts three on ONE child);
//! - an ordinary, unflagged interface measures 0 Hz until MORE than `IC_DEQUE_MIN_SAMPLE` = 2
//!   samples exist (`RNS/Interfaces/Interface.py:84`, `:346-348`): two announces 100 ms apart
//!   on a fresh plain TCP link are both learned, the third within the same second is held;
//! - the held third is released after `burst_hold` 15 s + `burst_penalty` 15 s under
//!   `AnnounceRateLimit::default()` (`Interface.py:76-84`): it is NOT learned inside 3 s and IS
//!   learned well inside 50 s (the previous defaults, 60 s + 300 s, would hold it for six
//!   minutes).

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
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// Spacing well inside one second: three announces at this spacing read 15 Hz on an ordinary
/// fresh interface, over the 3 Hz `burst_freq_new` threshold.
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// The window in which a held announce must NOT surface (the hold alone is 15 s).
const HELD: Duration = Duration::from_secs(3);
/// The window in which the held announce MUST surface: hold 15 s + penalty 15 s + release
/// tick slack. Six minutes under the previous defaults.
const RELEASED: Duration = Duration::from_secs(50);
const APP_NAME: &str = "announce-admission-probe";

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

/// Waits up to `within` for the announce of `hash`; `Ok(elapsed)` when it arrived, `Err(())`
/// when the window passed without it.
#[deny(clippy::await_holding_invalid_type)]
async fn announce_within(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
    within: Duration,
) -> Result<Duration, ()> {
    let started = Instant::now();
    timeout(within, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if address_hash == *hash {
                        return started.elapsed();
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .map_err(|_| ())
}

#[deny(clippy::await_holding_invalid_type)]
async fn learn_announce(
    what: &str,
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> Duration {
    announce_within(announces, hash, WAIT)
        .await
        .unwrap_or_else(|_| panic!("{what}: the announce did not arrive within {WAIT:?}"))
}

/// Waits up to `WAIT` for the announces of every hash in `expected`, in whatever order they
/// arrive (a transport announces its destination map in map order, not registration order);
/// returns the hashes still missing when the window passed.
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

/// A transport-enabled node with a `TcpServer` on 127.0.0.1, flagged as the shared instance
/// when asked (rnsd's topology) and left an ordinary listener otherwise.
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_server(name: &str, shared_instance: bool) -> (Transport, SocketAddr) {
    let port = free_loopback_port().await;
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
    (transport, SocketAddr::from(([127, 0, 0, 1], port)))
}

/// A local client of the shared instance attached with `SharedInstanceOptions::default()`,
/// its announce receiver subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn local_client(
    name: &str,
    endpoint: SocketAddr,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the attach is accepted");
    (transport, announces)
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

/// Announces `destination` on `transport` over its ordinary interfaces.
#[deny(clippy::await_holding_invalid_type)]
async fn announce(transport: &Transport, destination: &Arc<Mutex<SingleInputDestination>>) {
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
}

/// A plain, unflagged `TcpClient` connector to `endpoint`; its announce receiver is
/// subscribed before the connect, and the call returns once the listener accepted it.
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

/// One local client announces three destinations back to back (all three in one call) and
/// another local client, attached first, learns all three within `WAIT`, in whatever order
/// they were sent. Three announces land on ONE per-client child of the shared instance and
/// on the learner's own shared-instance interface; both are exempt
/// (`RNS/Interfaces/LocalInterface.py:130`), so neither the second nor the third is held (a
/// hold would last 30 s, past `WAIT`) and no `holding announce` record is logged.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_local_client_announcing_three_destinations_back_to_back_is_learned_in_full() {
    install_hold_logger();
    let names = ["probe-shared-instance", "probe-learner", "probe-announcer"];
    let (shared, endpoint) = tcp_server("probe-shared-instance", true).await;
    let (learner, mut learner_announces) = local_client("probe-learner", endpoint).await;
    let (announcer, _announcer_announces) = local_client("probe-announcer", endpoint).await;

    let destinations = register_destinations(&announcer, 3).await;
    assert_eq!(announcer.announce_destinations_to_shared_instance().await, 3);
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut learner_announces, &hashes).await;
    assert!(
        missing.is_empty(),
        "the learner did not learn {} of the three destinations within {WAIT:?} ({:?}); hold \
         records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    for (idx, (_destination, hash)) in destinations.iter().enumerate() {
        assert!(
            learner.destination_desc(hash).await.is_some(),
            "the learner resolves d{idx} after the burst"
        );
    }
    eprintln!("all three learned after {:?}", started.elapsed());
    assert!(
        holds_by(&names).is_empty(),
        "a shared-instance interface held an announce: {:?}",
        holds_by(&names)
    );

    drop(destinations);
    stop_all(vec![announcer, learner, shared]).await;
}

/// On an ordinary fresh interface the measured frequency reads 0 until more than two samples
/// exist (`RNS/Interfaces/Interface.py:84`, `:346-348`): two announces 100 ms apart from an
/// unflagged listener are both learned by a plain connector without spacing.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn ordinary_fresh_interface_admits_two_announces_without_spacing() {
    let (listener, endpoint) = tcp_server("probe-plain-listener", false).await;
    let (connector, mut announces) = plain_connector("probe-plain-connector", endpoint).await;
    let destinations = register_destinations(&listener, 2).await;

    announce(&listener, &destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&listener, &destinations[1].0).await;

    let first = learn_announce("the connector learns d0", &mut announces, &destinations[0].1).await;
    let second = learn_announce(
        "the connector learns d1, 100 ms after d0",
        &mut announces,
        &destinations[1].1,
    )
    .await;
    eprintln!("d0 after {first:?}, d1 after {second:?}");

    drop(destinations);
    stop_all(vec![connector, listener]).await;
}

/// Three announces inside one second on an ordinary fresh interface trip the limiter on the
/// third (15 Hz over the 3 Hz `burst_freq_new`), which is held and then released after
/// `burst_hold` + `burst_penalty` = 15 s + 15 s under the default limiter: the third
/// destination is not learned within 3 s and is learned within 50 s. Under the previous
/// defaults (60 s + 300 s) it would surface after six minutes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn ordinary_fresh_interface_holds_the_third_announce_for_hold_plus_penalty() {
    let (listener, endpoint) = tcp_server("probe-held-listener", false).await;
    let (connector, mut announces) = plain_connector("probe-held-connector", endpoint).await;
    let destinations = register_destinations(&listener, 3).await;

    announce(&listener, &destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&listener, &destinations[1].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    let third_sent = Instant::now();
    announce(&listener, &destinations[2].0).await;

    learn_announce("the connector learns d0", &mut announces, &destinations[0].1).await;
    learn_announce("the connector learns d1", &mut announces, &destinations[1].1).await;

    let early = announce_within(&mut announces, &destinations[2].1, HELD).await;
    assert!(
        early.is_err(),
        "the third announce inside one second was admitted after {:?}; the limiter must hold it",
        early.unwrap_or_default()
    );
    let remaining = RELEASED.saturating_sub(third_sent.elapsed());
    let released = announce_within(&mut announces, &destinations[2].1, remaining).await;
    assert!(
        released.is_ok(),
        "the held third announce was not released within {RELEASED:?} of being sent \
         (hold 15 s + penalty 15 s under the default limiter)"
    );
    let held_for = third_sent.elapsed();
    eprintln!("the held third announce surfaced {held_for:?} after it was sent");
    assert!(
        held_for >= Duration::from_secs(25),
        "the release came after {held_for:?}, before the 15 s hold and 15 s penalty could elapse"
    );
    assert!(
        connector.destination_desc(&destinations[2].1).await.is_some(),
        "the connector resolves the released destination"
    );

    drop(destinations);
    stop_all(vec![connector, listener]).await;
}
