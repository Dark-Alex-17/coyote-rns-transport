//! Announce ingress admission exercised from outside the crate over real TCP:
//!
//! - a shared-instance interface never ingress-limits, on the client's side AND on the child a
//!   shared instance holds per accepted client (`RNS/Interfaces/LocalInterface.py:130`): one
//!   local client announcing THREE destinations back to back, with no packet context, is
//!   learned in full by another local client, so neither the server's per-client child nor
//!   the learner's own interface holds the second or third
//!   (`third_client_admits_two_local_client_announces_back_to_back` in
//!   `shared_instance_one_identity_two_clients_probe.rs` spreads two announces over two
//!   children; this one puts three on ONE child). A path-response announce, the kind
//!   `announce_destinations_to_shared_instance` sends, is admitted before the exemption is
//!   consulted, so it would not exercise the child's half; registering a destination on an
//!   attached client also spawns one such announce 250 ms later, so the child sees two
//!   announces per destination;
//! - an ordinary, unflagged interface measures 0 Hz until MORE than `IC_DEQUE_MIN_SAMPLE` = 2
//!   samples exist (`RNS/Interfaces/Interface.py:84`, `:346-348`): two announces 100 ms apart
//!   on a fresh plain TCP link are both learned, the third within the same second is held;
//! - the held third is released after `burst_hold` 15 s + `burst_penalty` 15 s under
//!   `AnnounceRateLimit::default()` (`Interface.py:76-84`): it is NOT learned inside 3 s and IS
//!   learned well inside 50 s (the previous defaults, 60 s + 300 s, would hold it for six
//!   minutes).
//! - the exemption is read before any rate limit is looked up: a local client whose
//!   shared-instance interface carries an explicit `ingress_control = true` and thresholds that
//!   hold on any measurable frequency still learns three back-to-back announces in full, as
//!   Python's `LocalInterface` cannot be ingress-limited by configuration;
//! - the server-side child is exempt on its own: a plain, unflagged `TcpClient` connector of a
//!   flagged listener announces three destinations back to back and an attached local client
//!   learns all three (the only limiter on that path is the child's);
//! - the per-interface `ic_*` overrides keep applying over the refreshed defaults.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceSharedConfig;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::broadcast;

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
/// Spacing well inside one second: three announces at this spacing read 15 Hz on an ordinary
/// fresh interface, over the 3 Hz `burst_freq_new` threshold.
const BACK_TO_BACK: Duration = Duration::from_millis(100);
/// The window in which a held announce must NOT surface (the hold alone is 15 s).
const HELD: Duration = Duration::from_secs(3);
/// The window in which the held announce MUST surface: hold 15 s + penalty 15 s + release
/// tick slack. Six minutes under the previous defaults.
const RELEASED: Duration = Duration::from_secs(50);
const APP_NAME: &str = "announce-admission-probe";

/// The transport names of one flagged listener and its two peers. `holds_by` filters the
/// hold log by the same names the transports are built with, so the two cannot drift apart.
struct Names {
    shared_instance: &'static str,
    learner: &'static str,
    announcer: &'static str,
}

impl Names {
    fn all(&self) -> [&'static str; 3] {
        [self.shared_instance, self.learner, self.announcer]
    }
}

const BURST_NAMES: Names = Names {
    shared_instance: "probe-shared-instance",
    learner: "probe-learner",
    announcer: "probe-announcer",
};
const INGRESS_CONTROL_NAMES: Names = Names {
    shared_instance: "probe-ic-shared-instance",
    learner: "probe-ic-learner",
    announcer: "probe-ic-announcer",
};
const CHILD_NAMES: Names = Names {
    shared_instance: "probe-child-shared-instance",
    learner: "probe-child-learner",
    announcer: "probe-child-connector",
};

#[deny(clippy::await_holding_invalid_type)]
async fn expect_announce_within(
    what: &str,
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> Duration {
    announce_within(announces, hash, WAIT)
        .await
        .unwrap_or_else(|_| panic!("{what}: the announce did not arrive within {WAIT:?}"))
}

/// `tcp_server_on` on a port the loopback stack has just released; returns the endpoint a
/// client dials as well.
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_server(name: &str, shared_instance: bool) -> (Transport, SocketAddr, AddressHash) {
    let port = free_loopback_port().await;
    let (transport, listener) = tcp_server_on(name, port, shared_instance).await;
    (transport, SocketAddr::from(([127, 0, 0, 1], port)), listener)
}

/// A local client of the shared instance attached with `SharedInstanceOptions::default()`,
/// its announce receiver subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn local_client(
    name: &str,
    endpoint: SocketAddr,
) -> (Transport, broadcast::Receiver<AnnounceEvent>, AddressHash) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the attach is accepted");
    (transport, announces, iface)
}

/// A plain, unflagged `TcpClient` connector to `endpoint`, with `shared` applied to its
/// interface before the spawn when given; its announce receiver is subscribed before the
/// connect, and the call returns once the listener accepted it.
#[deny(clippy::await_holding_invalid_type)]
async fn plain_connector(
    name: &str,
    endpoint: SocketAddr,
    shared: Option<InterfaceSharedConfig>,
) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(endpoint.to_string());
    let status = client.runtime_status_handle();
    let context = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        let context = manager.new_context(client);
        if let Some(shared) = shared {
            assert!(
                manager.set_shared_config(*context.channel.address(), shared),
                "shared config applied to the connector iface"
            );
        }
        context
    };
    tokio::spawn(TcpClient::spawn(context));
    wait_until_within("the connector to connect", WAIT, || {
        status.to_json()["stream_state"].as_str() == Some("connected")
    })
    .await;
    (transport, announces)
}

/// One local client announces three destinations back to back with no packet context and
/// another local client, attached first, learns all three within `WAIT`, in whatever order
/// they were sent. Three announces land on ONE per-client child of the shared instance and
/// on the learner's own shared-instance interface, and the three registration announces
/// follow 250 ms later; both interfaces are exempt (`RNS/Interfaces/LocalInterface.py:130`),
/// so neither the second nor the third is held (a hold would last 30 s, past `WAIT`) and no
/// `holding announce` record is logged.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_local_client_announcing_three_destinations_back_to_back_is_learned_in_full() {
    install_hold_logger();
    let names = BURST_NAMES.all();
    let (shared, endpoint, _) = tcp_server(BURST_NAMES.shared_instance, true).await;
    let (learner, mut learner_announces, _) = local_client(BURST_NAMES.learner, endpoint).await;
    let (announcer, _announcer_announces, _) = local_client(BURST_NAMES.announcer, endpoint).await;

    let destinations = register_destinations(&announcer, APP_NAME, "d", 3).await;
    for (destination, _hash) in &destinations {
        announce(&announcer, destination).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let started = Instant::now();
    let missing = learn_all(&mut learner_announces, &hashes, WAIT).await;
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
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "all three were learned only after {:?}; a held announce would surface no sooner than \
         30 s",
        started.elapsed()
    );
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
    let (listener, endpoint, _) = tcp_server("probe-plain-listener", false).await;
    let (connector, mut announces) = plain_connector("probe-plain-connector", endpoint, None).await;
    let destinations = register_destinations(&listener, APP_NAME, "d", 2).await;

    announce(&listener, &destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&listener, &destinations[1].0).await;

    let first =
        expect_announce_within("the connector learns d0", &mut announces, &destinations[0].1).await;
    let second = expect_announce_within(
        "the connector learns d1, 100 ms after d0",
        &mut announces,
        &destinations[1].1,
    )
    .await;
    assert!(
        first < Duration::from_secs(5) && second < Duration::from_secs(5),
        "d0 after {first:?}, d1 after {second:?}; a held announce would surface no sooner \
         than 30 s"
    );

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
    install_hold_logger();
    let (listener, endpoint, _) = tcp_server("probe-held-listener", false).await;
    let (connector, mut announces) = plain_connector("probe-held-connector", endpoint, None).await;
    let destinations = register_destinations(&listener, APP_NAME, "d", 3).await;

    announce(&listener, &destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&listener, &destinations[1].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    let third_sent = Instant::now();
    announce(&listener, &destinations[2].0).await;

    expect_announce_within("the connector learns d0", &mut announces, &destinations[0].1).await;
    expect_announce_within("the connector learns d1", &mut announces, &destinations[1].1).await;

    let early = announce_within(&mut announces, &destinations[2].1, HELD).await;
    assert!(
        early.is_err(),
        "the third announce inside one second was admitted after {:?}; the limiter must hold it",
        early.unwrap_or_default()
    );
    // The positive control for the empty `holds_by` checks of the exemption pins: the hold
    // is logged under the connector's name with the text they filter on.
    assert!(
        !holds_by(&["probe-held-connector"]).is_empty(),
        "the connector held the third announce but logged no holding announce record"
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

/// An ingress config that would hold on any measurable frequency, with `ingress_control`
/// switched on explicitly.
fn hold_everything() -> InterfaceSharedConfig {
    InterfaceSharedConfig {
        ingress_control: Some(true),
        ic_burst_freq_new: Some(0.000_1),
        ic_burst_freq: Some(0.000_1),
        ..InterfaceSharedConfig::default()
    }
}

/// The exemption is decided before any rate limit is looked up: a local client whose
/// shared-instance interface carries `ingress_control = true` and thresholds that hold on any
/// measurable frequency, attached to a flagged listener whose own interface carries the same
/// config, still learns three back-to-back announces, sent with no packet context, from
/// another local client with no `holding announce` record. Python's `LocalInterface` sets
/// `ingress_control = False` in its constructor (`RNS/Interfaces/LocalInterface.py:130`); no
/// configuration limits it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_interfaces_ignore_an_explicit_ingress_control_true() {
    install_hold_logger();
    let names = INGRESS_CONTROL_NAMES.all();
    let (shared, endpoint, listener) =
        tcp_server(INGRESS_CONTROL_NAMES.shared_instance, true).await;
    {
        let manager = shared.iface_manager();
        let mut manager = manager.lock().await;
        assert!(manager.set_shared_config(listener, hold_everything()), "listener config");
    }
    let (learner, mut learner_announces, learner_iface) =
        local_client(INGRESS_CONTROL_NAMES.learner, endpoint).await;
    {
        let manager = learner.iface_manager();
        let mut manager = manager.lock().await;
        assert!(manager.is_shared_instance(&learner_iface), "the attach flags the client iface");
        assert!(manager.set_shared_config(learner_iface, hold_everything()));
    }
    let (announcer, _announcer_announces, _) =
        local_client(INGRESS_CONTROL_NAMES.announcer, endpoint).await;

    let destinations = register_destinations(&announcer, APP_NAME, "d", 3).await;
    for (destination, _hash) in &destinations {
        announce(&announcer, destination).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let missing = learn_all(&mut learner_announces, &hashes, WAIT).await;
    assert!(
        missing.is_empty(),
        "the learner did not learn {} of the three destinations within {WAIT:?} ({:?}); hold \
         records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    assert!(
        holds_by(&names).is_empty(),
        "a shared-instance interface with ingress_control = true held an announce: {:?}",
        holds_by(&names)
    );

    drop(destinations);
    stop_all(vec![announcer, learner, shared]).await;
}

/// The server-side child is exempt on its own: a plain, unflagged `TcpClient` connector of a
/// flagged listener (Python spawns a `LocalClientInterface` for any accepted connection)
/// announces three destinations back to back, and a local client attached to the listener
/// learns all three. The connector's own interface sees no inbound announce, so the only
/// limiter on the path is the one the listener's per-client child would run.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn plain_connector_announcing_three_to_a_flagged_listener_is_learned_in_full() {
    install_hold_logger();
    let names = CHILD_NAMES.all();
    let (shared, endpoint, _) = tcp_server(CHILD_NAMES.shared_instance, true).await;
    let (learner, mut learner_announces, _) = local_client(CHILD_NAMES.learner, endpoint).await;
    let (connector, _connector_announces) =
        plain_connector(CHILD_NAMES.announcer, endpoint, None).await;

    let destinations = register_destinations(&connector, APP_NAME, "d", 3).await;
    for (destination, _hash) in &destinations {
        announce(&connector, destination).await;
    }
    let hashes: Vec<AddressHash> = destinations.iter().map(|(_, hash)| *hash).collect();

    let missing = learn_all(&mut learner_announces, &hashes, WAIT).await;
    assert!(
        missing.is_empty(),
        "the local client did not learn {} of the connector's three destinations within \
         {WAIT:?} ({:?}); hold records: {:?}",
        missing.len(),
        missing,
        holds_by(&names)
    );
    assert!(
        holds_by(&names).is_empty(),
        "the shared instance's child held an unflagged connector's announce: {:?}",
        holds_by(&names)
    );

    drop(destinations);
    stop_all(vec![connector, learner, shared]).await;
}

/// The per-interface `ic_*` overrides keep applying over the refreshed defaults: with
/// `ic_burst_hold = 1`, `ic_burst_penalty = 1` and `ic_held_release_interval = 1` on an
/// ordinary connector interface, the third of three announces 100 ms apart is still held (not
/// learned inside 1 s) but surfaces within 10 s rather than the default 30 s.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn ordinary_interface_ic_overrides_shorten_the_hold() {
    install_hold_logger();
    let (listener, endpoint, _) = tcp_server("probe-override-listener", false).await;
    let overrides = InterfaceSharedConfig {
        ic_burst_hold: Some(1.0),
        ic_burst_penalty: Some(1.0),
        ic_held_release_interval: Some(1.0),
        ..InterfaceSharedConfig::default()
    };
    let (connector, mut announces) =
        plain_connector("probe-override-connector", endpoint, Some(overrides)).await;
    let destinations = register_destinations(&listener, APP_NAME, "d", 3).await;

    announce(&listener, &destinations[0].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    announce(&listener, &destinations[1].0).await;
    tokio::time::sleep(BACK_TO_BACK).await;
    let third_sent = Instant::now();
    announce(&listener, &destinations[2].0).await;

    expect_announce_within("the connector learns d0", &mut announces, &destinations[0].1).await;
    expect_announce_within("the connector learns d1", &mut announces, &destinations[1].1).await;

    let early = announce_within(&mut announces, &destinations[2].1, Duration::from_secs(1)).await;
    assert!(
        early.is_err(),
        "the third announce inside one second was admitted after {:?}; the limiter must hold \
         it for at least ic_burst_hold = 1 s",
        early.unwrap_or_default()
    );
    assert!(
        !holds_by(&["probe-override-connector"]).is_empty(),
        "the connector held the third announce but logged no holding announce record"
    );
    let released =
        announce_within(&mut announces, &destinations[2].1, Duration::from_secs(10)).await;
    let held_for = third_sent.elapsed();
    assert!(
        released.is_ok(),
        "the held third announce was not released within 10 s of being sent under \
         ic_burst_hold = 1 s, ic_burst_penalty = 1 s, ic_held_release_interval = 1 s"
    );
    eprintln!("under the 1 s overrides the held third announce surfaced after {held_for:?}");
    assert!(
        held_for >= Duration::from_secs(2),
        "the release came after {held_for:?}, before the 1 s hold and 1 s penalty could elapse"
    );
    assert!(
        connector.destination_desc(&destinations[2].1).await.is_some(),
        "the connector resolves the released destination"
    );

    drop(destinations);
    stop_all(vec![connector, listener]).await;
}
