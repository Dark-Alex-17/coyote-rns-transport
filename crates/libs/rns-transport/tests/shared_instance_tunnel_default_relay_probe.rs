//! What a consumer gets from a default-option shared-instance attach now that the attach
//! synthesizes no tunnel, run from outside the crate, where the shared instance's tunnel
//! table is not visible (`shared_instance_tunnel_synthesis_api.rs` pins that the default
//! writes no tunnel packet on the wire). Two clients attached with
//! `SharedInstanceOptions::default()` to a Rust `TcpServer` shared instance still learn each
//! other's announces and open links both ways, and after the shared instance is stopped and
//! started again on its port both clients reconnect, re-announce on their own, learn each
//! other again the way the shape calls for and open a link again, all without the consumer
//! ever touching `with_tunnel_synthesis`. The tunnel was never what carried the relay or the
//! client-side hop count (`RNS/Transport.py:1416`, `:1937-1940` key on the posture flags),
//! and it never carried a client's own path table either: a reconnected client learns its
//! peer again from an announce, opted in or not.
//!
//! Two shapes of shared instance, since the README documents both: the plain
//! transport-enabled `TcpServer`, which reads a local client's announce at one hop and
//! rebroadcasts a plain announce as any transport node would (path responses are not
//! rebroadcast, `RNS/Transport.py:2351`), and the listener flagged as the shared instance
//! the way rnsd and reticulumd flag theirs, which reads the announce at zero hops and fans
//! a local client's path-response announce out to every other local client at once
//! (`RNS/Transport.py:2398-2427`).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{LinkEvent, LinkEventData};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
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

/// The shared instance: a transport-enabled node with a `TcpServer` on 127.0.0.1, its
/// listener flagged as the shared instance when `flagged`.
struct SharedInstance {
    transport: Transport,
    endpoint: SocketAddr,
    flagged: bool,
}

/// One default-option client: its transport, its state stream, its announce receiver and,
/// once `register` ran, the destination it registered.
struct Client {
    transport: Transport,
    events: mpsc::Receiver<SharedInstanceEvent>,
    announces: broadcast::Receiver<AnnounceEvent>,
    destination: Option<Arc<Mutex<SingleInputDestination>>>,
    destination_hash: AddressHash,
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
async fn shared_instance_on(port: u16, flagged: bool) -> SharedInstance {
    let mut config = TransportConfig::new(
        "tunnel-default-relay-shared-instance",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    );
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    let listener = transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    if flagged {
        assert!(
            transport.iface_manager().lock().await.set_shared_instance(listener, true),
            "the listener is flagged as the shared instance"
        );
    }
    wait_until("the shared instance to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    SharedInstance { transport, endpoint: SocketAddr::from(([127, 0, 0, 1], port)), flagged }
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}

/// Reads the state stream until `Connected`: a client that redials at once while the port
/// is still closed reports `DialFailed` (and `Connecting` again) first, which is the stream
/// doing its job, not a failure of the reconnect.
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

/// Attaches a fresh-identity client with `SharedInstanceOptions::default()` (plus the
/// state stream and a short reconnect wait, neither of which touches the tunnel option)
/// and reads `Connecting` then `Connected`. No destination yet: a shared instance relays an
/// announce only to the clients attached when it arrives, so both clients attach before
/// either registers.
#[deny(clippy::await_holding_invalid_type)]
async fn default_client(name: &str, endpoint: SocketAddr) -> Client {
    let transport =
        Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false));
    let announces = transport.recv_announces().await;
    let (events_tx, mut events) = mpsc::channel(16);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx),
        )
        .await
        .expect("the default attach is accepted");
    assert_eq!(next_event(name, &mut events).await, Some(SharedInstanceEvent::Connecting));
    assert_eq!(next_event(name, &mut events).await, Some(SharedInstanceEvent::Connected));
    Client {
        transport,
        events,
        announces,
        destination: None,
        destination_hash: AddressHash::new_empty(),
    }
}

/// Registers one destination under `aspect`; while attached, the client announces it as
/// a path response on its own 0.25 s later (`RNS/Transport.py:2911-2916`).
#[deny(clippy::await_holding_invalid_type)]
async fn register(client: &mut Client, aspect: &str) {
    let destination = client
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("tunnel-default-relay", aspect),
        )
        .await;
    client.destination_hash = destination.lock().await.desc.address_hash;
    client.destination = Some(destination);
}

/// Announces the client's destination the way the shared instance's shape calls for: on
/// demand as a path response to a flagged shared instance, which fans it out to the other
/// local clients; as a plain announce to a plain server, which rebroadcasts it as any
/// transport node would.
#[deny(clippy::await_holding_invalid_type)]
async fn announce(client: &Client, flagged: bool) {
    if flagged {
        assert_eq!(client.transport.announce_destinations_to_shared_instance().await, 1);
    } else {
        let destination = client.destination.as_ref().expect("registered");
        let packet = destination.lock().await.announce(OsRng, None).expect("valid announce");
        client.transport.send_packet(packet).await;
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn learn_announce(
    what: &str,
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
) -> AnnounceEvent {
    timeout(WAIT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    let address_hash = event.destination.lock().await.desc.address_hash;
                    if address_hash == *hash {
                        return event;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: announce channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("{what}: the announce did not arrive"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn await_activated(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    what: &str,
) {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.id == link_id && matches!(event.event, LinkEvent::Activated) => {
                    return;
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("{what}: link event channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// `from` opens a link to `to`'s destination; it activates on both ends.
#[deny(clippy::await_holding_invalid_type)]
async fn open_link_both_ends(from: &Client, to: &Client, what: &str) {
    let mut in_events = to.transport.in_link_events();
    let mut out_events = from.transport.out_link_events();
    let desc = from
        .transport
        .destination_desc(&to.destination_hash)
        .await
        .unwrap_or_else(|| panic!("{what}: the destination was learned"));
    let link = from.transport.link(desc).await.unwrap_or_else(|err| panic!("{what}: {err:?}"));
    let link_id = *link.lock().await.id();
    await_activated(&mut in_events, link_id, &format!("{what} to activate on the target")).await;
    await_activated(&mut out_events, link_id, &format!("{what} to activate on the opener")).await;
}

/// Each client announces; the other learns it, at one hop through a plain server (the
/// rebroadcast of a one-hop path). Through a flagged shared instance the peer's hop count is
/// Python's local-client arithmetic and is not pinned here.
#[deny(clippy::await_holding_invalid_type)]
async fn announce_and_learn_both_ways(a: &mut Client, b: &mut Client, flagged: bool, round: &str) {
    announce(a, flagged).await;
    announce(b, flagged).await;
    let at_b =
        learn_announce(&format!("{round}: b learns a"), &mut b.announces, &a.destination_hash)
            .await;
    let at_a =
        learn_announce(&format!("{round}: a learns b"), &mut a.announces, &b.destination_hash)
            .await;
    if !flagged {
        assert_eq!(at_b.hops, 1, "{round}: b learns a's destination at one hop");
        assert_eq!(at_a.hops, 1, "{round}: a learns b's destination at one hop");
    }
}

/// After a drop the clients hold no paths (a dropped connection clears the path table, as
/// Python's `shared_connection_disappeared` does, `RNS/Transport.py:3690-3702`), and a plain
/// server does not rebroadcast the path-response re-announce, so the consumer announces
/// again. An announce for a destination the server already holds at the same hop count is
/// rebroadcast only when it is newer than the held one, and its timestamp is a whole second
/// (`RNS/Identity.py` packs `announce_emitted` as an integer), so the round is repeated on a
/// cadence longer than a second until both peers learned each other at one hop.
#[deny(clippy::await_holding_invalid_type)]
async fn reannounce_until_learned_both_ways(a: &mut Client, b: &mut Client, round: &str) {
    let deadline = Instant::now() + WAIT;
    let window = Duration::from_millis(1200);
    loop {
        announce(a, false).await;
        announce(b, false).await;
        let learned = timeout(window, async {
            let at_b = learn_announce(round, &mut b.announces, &a.destination_hash).await;
            let at_a = learn_announce(round, &mut a.announces, &b.destination_hash).await;
            (at_b.hops, at_a.hops)
        })
        .await;
        if let Ok((hops_at_b, hops_at_a)) = learned {
            assert_eq!(hops_at_b, 1, "{round}: b learns a's destination at one hop again");
            assert_eq!(hops_at_a, 1, "{round}: a learns b's destination at one hop again");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{round}: the peers did not learn each other again within {WAIT:?}"
        );
    }
}

/// Two default-option clients relay announces and open links both ways; the shared
/// instance stopped and started again on its port is followed by `Disconnected`,
/// `Connecting`, `Connected` on each client, each client's unprompted re-announce reaching
/// the restarted instance as a path response, and a link opening again.
#[deny(clippy::await_holding_invalid_type)]
async fn default_clients_relay_and_survive_a_restart(flagged: bool) {
    let port = free_loopback_port().await;
    let shared = shared_instance_on(port, flagged).await;
    let mut shared_announces = shared.transport.recv_announces().await;
    let mut a = default_client("tunnel-default-relay-a", shared.endpoint).await;
    let mut b = default_client("tunnel-default-relay-b", shared.endpoint).await;
    assert!(a.transport.connected_to_shared_instance().await, "a holds the client posture");
    assert!(b.transport.connected_to_shared_instance().await, "b holds the client posture");
    register(&mut a, "a").await;
    register(&mut b, "b").await;
    assert_ne!(a.destination_hash, b.destination_hash, "two distinct destinations");

    announce_and_learn_both_ways(&mut a, &mut b, flagged, "first round").await;
    let at_shared =
        learn_announce("the shared instance learns a", &mut shared_announces, &a.destination_hash)
            .await;
    // README "Attaching to a shared instance": the plain server reads a local client's
    // announce at one hop, a flagged listener at zero (`RNS/Transport.py:1938`).
    assert_eq!(at_shared.hops, u8::from(!shared.flagged), "the shared instance's hop count");
    open_link_both_ends(&b, &a, "the link b -> a").await;
    open_link_both_ends(&a, &b, "the link a -> b").await;

    // A forced server-side drop: the shared instance goes away and comes back on its port.
    timeout(WAIT, shared.transport.stop()).await.expect("shared instance stop completes");
    for (name, client) in [("a", &mut a), ("b", &mut b)] {
        assert_eq!(
            next_event(name, &mut client.events).await,
            Some(SharedInstanceEvent::Disconnected),
            "{name}: the drop is served"
        );
        assert_eq!(
            next_event(name, &mut client.events).await,
            Some(SharedInstanceEvent::Connecting),
            "{name}: `Connecting` follows the drop"
        );
    }
    let restarted = shared_instance_on(port, flagged).await;
    let mut restarted_announces = restarted.transport.recv_announces().await;
    for (name, client) in [("a", &mut a), ("b", &mut b)] {
        await_connected(
            &format!("{name}: the redial on the restarted instance"),
            &mut client.events,
        )
        .await;
    }

    // The clients re-announce on their own, `reconnect_wait` plus 2 s after the reconnect.
    let reannounced_a = learn_announce(
        "the restarted instance learns a again",
        &mut restarted_announces,
        &a.destination_hash,
    )
    .await;
    assert!(reannounced_a.is_path_response, "the re-announce is a path response");
    learn_announce(
        "the restarted instance learns b again",
        &mut restarted_announces,
        &b.destination_hash,
    )
    .await;
    // A flagged shared instance fans that re-announce out to the other local client at once.
    // A plain server does not rebroadcast a path response (`RNS/Transport.py:2351`), and a
    // reconnected client holds no path to its peer any more (`Transport::link` is `NoPath`
    // until it hears an announce again; the same with the tunnel opted in, see the module
    // doc), so on that shape the consumer announces again until the peer is learned.
    if flagged {
        learn_announce("after the restart: b learns a", &mut b.announces, &a.destination_hash)
            .await;
        learn_announce("after the restart: a learns b", &mut a.announces, &b.destination_hash)
            .await;
    } else {
        reannounce_until_learned_both_ways(&mut a, &mut b, "after the restart").await;
    }
    open_link_both_ends(&b, &a, "the link b -> a after the restart").await;
    open_link_both_ends(&a, &b, "the link a -> b after the restart").await;

    drop(a.destination.take());
    drop(b.destination.take());
    assert!(a.transport.detach_shared_instance().await, "a is still attached");
    assert!(b.transport.detach_shared_instance().await, "b is still attached");
    timeout(WAIT, a.transport.stop()).await.expect("client a stop completes");
    timeout(WAIT, b.transport.stop()).await.expect("client b stop completes");
    timeout(WAIT, restarted.transport.stop()).await.expect("restarted instance stop completes");
}

/// Criterion (a)'s topology from outside the crate: a plain transport-enabled `TcpServer`,
/// two default-option clients, announces learned at one hop both ways, links both ways,
/// and the same after a forced restart.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn default_clients_on_a_plain_server_relay_at_one_hop_link_both_ways_and_survive_a_restart() {
    default_clients_relay_and_survive_a_restart(false).await;
}

/// The topology of rnsd and reticulumd: the listener flagged as the shared instance, the
/// on-demand path-response announce fanned out to the other local client, links both ways,
/// and the same after a forced restart.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn default_clients_on_a_flagged_shared_instance_relay_link_both_ways_and_survive_a_restart() {
    default_clients_relay_and_survive_a_restart(true).await;
}
