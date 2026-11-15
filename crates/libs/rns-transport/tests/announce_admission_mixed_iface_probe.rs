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
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// The window in which a held announce must NOT surface (the hold alone is 15 s).
const HELD: Duration = Duration::from_secs(3);
/// The window in which an exempt burst MUST be learned in full.
const PROMPT: Duration = Duration::from_secs(5);
const APP_NAME: &str = "announce-admission-mixed-probe";
const MIXED: &str = "probe-mixed-client";

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

/// The hold records logged by transport `name` for destination `hash`.
fn holds_for(name: &str, hash: &AddressHash) -> Vec<String> {
    let tp = format!("tp({name})");
    let dst = format!("{hash}");
    HOLDS
        .records
        .lock()
        .expect("hold record store")
        .iter()
        .filter(|message| message.contains(&tp) && message.contains(&dst))
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
async fn learn_all(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    expected: &[AddressHash],
    within: Duration,
) -> Vec<AddressHash> {
    let mut missing: Vec<AddressHash> = expected.to_vec();
    let deadline = Instant::now() + within;
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

/// A node with a `TcpServer` on 127.0.0.1, flagged as the shared instance when asked.
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
    wait_until("the plain connector to connect", || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn register_destinations(
    transport: &Transport,
    tag: &str,
    count: usize,
) -> Vec<(Arc<Mutex<SingleInputDestination>>, AddressHash)> {
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let destination = transport
            .add_destination(
                PrivateIdentity::new_from_rand(OsRng),
                DestinationName::new(APP_NAME, &format!("{tag}{idx}")),
            )
            .await;
        let hash = destination.lock().await.desc.address_hash;
        out.push((destination, hash));
    }
    out
}

/// Announces with no packet context, so the receiving limiter weighs it.
#[deny(clippy::await_holding_invalid_type)]
async fn announce(transport: &Transport, destination: &Arc<Mutex<SingleInputDestination>>) {
    let packet = destination.lock().await.announce(OsRng, None).expect("announce packet");
    transport.send_packet(packet).await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn stop_all(transports: Vec<Transport>) {
    for transport in transports {
        timeout(WAIT, transport.stop()).await.expect("stop completes");
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_transport_limits_its_plain_interface_and_exempts_its_shared_instance_interface() {
    install_hold_logger();
    let (shared, shared_endpoint) = tcp_server("probe-mixed-shared-instance", true).await;
    let (plain_listener, plain_endpoint) = tcp_server("probe-mixed-plain-listener", false).await;

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
    let plain = register_destinations(&plain_listener, "p", 3).await;
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
    let burst = register_destinations(&announcer, "s", 3).await;
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
        HOLDS
            .records
            .lock()
            .expect("hold record store")
            .iter()
            .filter(|message| message.contains(&format!("tp({MIXED})")))
            .count(),
        1,
        "the mixed transport logged a hold beyond the third plain announce"
    );

    drop(plain);
    drop(burst);
    stop_all(vec![announcer, mixed, plain_listener, shared]).await;
}
