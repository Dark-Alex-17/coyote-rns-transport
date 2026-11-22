//! The establishment timeout a consumer observes on a link opened over a shared-instance
//! client interface, read from outside the crate against the public API alone. Python
//! sizes it as
//! `first_hop_timeout + ESTABLISHMENT_TIMEOUT_PER_HOP * max(1, hops)` with
//! `first_hop_timeout = Reticulum.MTU * 8 / bitrate + DEFAULT_PER_HOP_TIMEOUT` at the FIXED
//! 500-byte `Reticulum.MTU` (`RNS/Link.py:282-283`, `RNS/Transport.py:3194-3197`,
//! `RNS/Reticulum.py:93`, `:142`), so a 1-hop path at 62500 bps times out at 12.064 s
//! whatever hardware MTU the interface carries. Before the fix the first hop was sized with
//! the next hop interface's hardware MTU, 262144 bytes on a shared-instance client, and the
//! same link stayed pending about 45 s.
//!
//! Shape: a transport-enabled Rust `TcpServer` S stands in for rnsd; H attaches, registers
//! a destination and announces it; C attaches with a 262144-byte MTU and a forced 62500 bps
//! bitrate (the figures the fix names) and learns the path at one hop. H stops, so nothing
//! answers a link request, and C's link closes with `LinkCloseReason::Timeout` no earlier
//! than Python's 12.0 s floor and no later than 12.064 s plus the maintenance job's slack.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::destination::link::LinkCloseReason;
use rns_transport::destination::DestinationName;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::mpsc;

#[path = "support/closed_event_for.rs"]
mod closed_event_for;
use closed_event_for::closed_event_for;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;
#[path = "support/learn_announce.rs"]
mod learn_announce;
use learn_announce::learn_announce;
#[path = "support/next_event.rs"]
mod next_event;
use next_event::next_event;
#[path = "support/stop_all.rs"]
mod stop_all;
use stop_all::stop_all;
#[path = "support/wait_until_within.rs"]
mod wait_until_within;
use wait_until_within::wait_until_within;

const WAIT: Duration = Duration::from_secs(15);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// The MTU the shared-instance client interface carries (the fix's own figure).
const CLIENT_MTU: usize = 262_144;
/// The bitrate the first hop is sized from; 500 * 8 / 62500 = 0.064 s.
const CLIENT_BITRATE: u64 = 62_500;
/// `DEFAULT_PER_HOP_TIMEOUT` and `ESTABLISHMENT_TIMEOUT_PER_HOP`, both 6 s.
const PER_HOP: Duration = Duration::from_secs(6);
/// Python's figure for a 1-hop path at 62500 bps: 6.064 + 6.
const EXPECTED_ONE_HOP: Duration = Duration::from_millis(12_064);
/// The maintenance job reads its clock on its own cadence; the live roaming part saw the
/// close 0.85 s past the figure.
const CLOSE_SLACK: Duration = Duration::from_millis(2_500);

#[deny(clippy::await_holding_invalid_type)]
async fn attach(name: &str, endpoint: SocketAddr, options: SharedInstanceOptions) -> Transport {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let (events_tx, mut events) = mpsc::channel(64);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            options.with_reconnect_wait(RECONNECT_WAIT).with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(next_event(name, &mut events, WAIT).await, Some(SharedInstanceEvent::Connecting));
    assert_eq!(next_event(name, &mut events, WAIT).await, Some(SharedInstanceEvent::Connected));
    transport
}

#[tokio::test]
async fn a_one_hop_link_over_a_262144_byte_shared_instance_client_times_out_at_pythons_twelve_seconds(
) {
    // S: a transport-enabled TcpServer, the plain shared-instance shape, which reads a
    // local client's announce at one hop and rebroadcasts it to the other clients.
    let mut config =
        TransportConfig::new("q3-shared-instance", &PrivateIdentity::new_from_rand(OsRng), false);
    config.set_transport_enabled(true);
    let shared = Transport::new(config);
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), shared.iface_manager());
    let status = server.runtime_status_handle();
    shared.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until_within("the shared instance to listen", WAIT, || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));

    // Both clients attach before the announce: a shared instance relays an announce only
    // to the clients attached when it arrives.
    let host = attach("q3-host", endpoint, SharedInstanceOptions::default()).await;
    let client = attach(
        "q3-client",
        endpoint,
        SharedInstanceOptions::default().with_mtu(CLIENT_MTU).with_forced_bitrate(CLIENT_BITRATE),
    )
    .await;
    let mut announces = client.recv_announces().await;
    let destination = host
        .add_destination(PrivateIdentity::new_from_rand(OsRng), DestinationName::new("q3", "host"))
        .await;
    let hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce");
    host.send_packet(packet).await;
    let learned = learn_announce("C learns H", &mut announces, &hash, WAIT).await;
    assert_eq!(learned.hops, 1, "one hop through a plain transport-enabled server");
    let hops = client.hops_to(&hash).await;
    assert_eq!(hops, 1);
    let expected = EXPECTED_ONE_HOP + PER_HOP * (hops.max(1) as u32 - 1);
    let desc = client.destination_desc(&hash).await.expect("C holds H's description");

    // H goes away: nothing answers the link request and C's own row stays.
    tokio::time::timeout(WAIT, host.stop()).await.expect("H stops");
    assert!(client.has_path(&hash).await, "C's row is its own, untouched by H leaving");

    let mut out_events = client.out_link_events();
    let started = Instant::now();
    let link = client.link(desc).await.expect("Ok: a path exists");
    let id = *link.lock().await.id();
    let closed = closed_event_for(&mut out_events, id, expected + CLOSE_SLACK).await;
    let elapsed = started.elapsed();
    eprintln!("OBSERVED: {hops}-hop link over a {CLIENT_MTU}-byte client interface at {CLIENT_BITRATE} bps closed with {:?} after {elapsed:?} (Python: {expected:?})", closed.close_reason);
    assert_eq!(closed.close_reason, Some(LinkCloseReason::Timeout));
    assert!(
        elapsed >= PER_HOP * 2,
        "Python's floor is first_hop_timeout (> 6 s) + 6 s per hop: closed after {elapsed:?}"
    );
    assert!(
        elapsed <= expected + CLOSE_SLACK,
        "sized with the 500-byte Reticulum MTU the link closes about {expected:?} in, \
         not the ~45 s a {CLIENT_MTU}-byte first hop gives: closed after {elapsed:?}"
    );

    stop_all(vec![client, shared]).await;
}
