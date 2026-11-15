//! Exercises the shared-instance default from outside the crate, the way a consumer sees
//! it: two transports built from ONE transport identity, each
//! registering a destination under a fresh identity, attach to a Rust `TcpServer` flagged as
//! the shared instance (rnsd's topology) with `SharedInstanceOptions::default()`. A third
//! client, attached before either announces (a shared instance relays a local client's
//! announce only to the clients attached when it arrives, `RNS/Transport.py:2400-2429`),
//! opens a link to the FIRST transport's destination after the second attached: it activates
//! on the first, and the second transport's `in_link_events()` stays empty, which the
//! in-crate twin (`src/transport/tests_parts/two_transport_shared_instance_tunnel_identity.rs`)
//! proves by activation alone and never reads. The symmetric link to the second's destination
//! activates on the second and leaves the first's in-link events empty.
//!
//! The negative control is the RED proof that the positive case discriminates, run from
//! outside the crate where the server's tunnel table is not visible: both opted in with
//! `.with_tunnel_synthesis(true)`, the two attaches carrying one interface hash (the counter
//! is per transport, so two first-interface attaches agree) and the first announced before the
//! second attaches, the shared instance takes the reappeared branch and re-points the first's
//! path at the newest connection (`RNS/Transport.py:2820-2868`): the third's link to the
//! first's destination reaches the second, which registered no such destination, and the
//! first's in-link events stay empty for 3 s while the third's out-link never activates.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{Link, LinkEvent, LinkEventData};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::{AddressHash, Hash};
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    AnnounceEvent, SharedInstanceEndpoint, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// The secondary window of the negative control: one hop to the shared instance and one to a
/// local client; a request that reaches the registering transport activates well inside it.
const NO_LINK: Duration = Duration::from_secs(3);
const APP_NAME: &str = "one-identity-two-clients";
/// Spacing between the first registrant's announce and the second attach. Adding a
/// destination on an attached client spawns a registration announce 250 ms later
/// (`SHARED_INSTANCE_REGISTRATION_ANNOUNCE_DELAY` in
/// `links_parts/transport_sections/shared_instance_announce.rs`, `RNS/Transport.py:2911-2916`).
/// The tunnel-synthesis negative control needs that re-announce to have landed before b
/// attaches, else it re-points a's path back and the control turns positive.
const ANNOUNCE_SPACING: Duration = Duration::from_millis(1100);
/// Back-to-back spacing a Python local client admits without a hold.
const BACK_TO_BACK: Duration = Duration::from_millis(100);

/// A local client of the shared instance: its transport, the interface its attach
/// registered, its announce receiver (subscribed before the attach) and the destination it
/// registered, if any.
struct Client {
    transport: Transport,
    iface: AddressHash,
    announces: broadcast::Receiver<AnnounceEvent>,
    in_links: broadcast::Receiver<LinkEventData>,
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

/// A transport-enabled node with a `TcpServer` on 127.0.0.1 whose listener is flagged as the
/// shared instance, the way rnsd and reticulumd flag theirs.
#[deny(clippy::await_holding_invalid_type)]
async fn flagged_shared_instance() -> (Transport, SocketAddr) {
    let port = free_loopback_port().await;
    let mut config = TransportConfig::new(
        "one-identity-shared-instance",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    );
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    let listener = transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    assert!(
        transport.iface_manager().lock().await.set_shared_instance(listener, true),
        "the listener is flagged as the shared instance"
    );
    wait_until("the shared instance to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    (transport, SocketAddr::from(([127, 0, 0, 1], port)))
}

/// Attaches a transport built under `identity` as a local client with `options`; the
/// announce and in-link receivers are subscribed before the attach.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(
    name: &str,
    identity: &PrivateIdentity,
    endpoint: SocketAddr,
    options: SharedInstanceOptions,
) -> Client {
    let transport = Transport::new(TransportConfig::new(name, identity, false));
    let announces = transport.recv_announces().await;
    let in_links = transport.in_link_events();
    let iface = transport
        .attach_shared_instance(SharedInstanceEndpoint::Tcp(endpoint), options)
        .await
        .expect("the attach is accepted");
    Client {
        transport,
        iface,
        announces,
        in_links,
        destination: None,
        destination_hash: AddressHash::new_empty(),
    }
}

/// Registers one destination under a FRESH identity (only the transport identity is shared)
/// and announces it to the shared instance as a path response, which a flagged shared
/// instance fans out to the other local clients. Every call produces two announces: the
/// immediate one and the registration announce 250 ms later.
#[deny(clippy::await_holding_invalid_type)]
async fn register_and_announce(client: &mut Client, aspect: &str) {
    let destination = client
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new(APP_NAME, aspect),
        )
        .await;
    client.destination_hash = destination.lock().await.desc.address_hash;
    client.destination = Some(destination);
    assert_eq!(client.transport.announce_destinations_to_shared_instance().await, 1);
}

/// The interface hash under which the client's shared-instance interface is registered; the
/// manager guard is held for the read alone.
#[deny(clippy::await_holding_invalid_type)]
async fn interface_hash(client: &Client) -> Option<Hash> {
    let manager = client.transport.iface_manager();
    let manager = manager.lock().await;
    manager.full_hash(&client.iface)
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

fn assert_no_in_link_event(events: &mut broadcast::Receiver<LinkEventData>, who: &str) {
    assert!(
        matches!(events.try_recv(), Err(TryRecvError::Empty)),
        "{who} saw an in-link event for a destination it did not register"
    );
}

/// `from` opens a link to the destination `hash` it learned; the handle is returned so the
/// caller keeps it alive beside the id.
#[deny(clippy::await_holding_invalid_type)]
async fn open_link(
    from: &Client,
    hash: &AddressHash,
    what: &str,
) -> (Arc<Mutex<Link>>, AddressHash) {
    let desc = from
        .transport
        .destination_desc(hash)
        .await
        .unwrap_or_else(|| panic!("{what}: the destination was learned"));
    let link = from.transport.link(desc).await.unwrap_or_else(|err| panic!("{what}: {err:?}"));
    let id = *link.lock().await.id();
    (link, id)
}

#[deny(clippy::await_holding_invalid_type)]
async fn stop_all(clients: Vec<Client>, shared: Transport) {
    for mut client in clients {
        drop(client.destination.take());
        timeout(WAIT, client.transport.stop()).await.expect("client stop completes");
    }
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
}

/// From outside the crate: under the default, the third client's link to the first
/// transport's destination activates on the first and the second's in-link events
/// stay empty; the symmetric link activates on the second and leaves the first's empty.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn default_attaches_under_one_identity_keep_each_link_on_the_registering_transport() {
    let (shared, endpoint) = flagged_shared_instance().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    // The third client first: it must be attached when a's announce is fanned out.
    let mut c = attach(
        "one-identity-c",
        &PrivateIdentity::new_from_rand(OsRng),
        endpoint,
        SharedInstanceOptions::default(),
    )
    .await;
    let mut a =
        attach("one-identity-a", &identity, endpoint, SharedInstanceOptions::default()).await;
    register_and_announce(&mut a, "a").await;
    learn_announce("c learns a", &mut c.announces, &a.destination_hash).await;

    // The second transport under the same identity attaches after a's path is filed.
    tokio::time::sleep(ANNOUNCE_SPACING).await;
    let mut b =
        attach("one-identity-b", &identity, endpoint, SharedInstanceOptions::default()).await;
    register_and_announce(&mut b, "b").await;
    assert_ne!(a.destination_hash, b.destination_hash, "two distinct destinations");
    learn_announce("c learns b", &mut c.announces, &b.destination_hash).await;

    let mut c_out = c.transport.out_link_events();
    let (_link_a, link_to_a) = open_link(&c, &a.destination_hash, "the link c -> a").await;
    await_activated(&mut a.in_links, link_to_a, "the link to a's destination to activate on a")
        .await;
    await_activated(&mut c_out, link_to_a, "the link to a's destination to activate on c").await;
    assert_no_in_link_event(&mut b.in_links, "b");

    let (_link_b, link_to_b) = open_link(&c, &b.destination_hash, "the link c -> b").await;
    await_activated(&mut b.in_links, link_to_b, "the link to b's destination to activate on b")
        .await;
    await_activated(&mut c_out, link_to_b, "the link to b's destination to activate on c").await;
    // a's stream carries only its own link's events.
    loop {
        match a.in_links.try_recv() {
            Ok(event) => {
                assert_eq!(event.id, link_to_a, "a saw an event for a link that is not its own")
            }
            Err(TryRecvError::Empty) => break,
            Err(err) => panic!("a's in-link channel failed: {err:?}"),
        }
    }

    stop_all(vec![a, b, c], shared).await;
}

/// The negative control from outside the crate, and why the default is off: with both
/// opted in and the first announced before the second attaches, the first's in-link events
/// stay empty for 3 s after the third's `link()` and the third's out-link never activates.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn opted_in_attaches_under_one_identity_move_the_first_transports_path_to_the_second() {
    let (shared, endpoint) = flagged_shared_instance().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let opted_in = || SharedInstanceOptions::default().with_tunnel_synthesis(true);
    let mut c = attach(
        "one-identity-on-c",
        &PrivateIdentity::new_from_rand(OsRng),
        endpoint,
        SharedInstanceOptions::default(),
    )
    .await;
    let mut a = attach("one-identity-on-a", &identity, endpoint, opted_in()).await;
    register_and_announce(&mut a, "a").await;
    learn_announce("c learns a", &mut c.announces, &a.destination_hash).await;

    tokio::time::sleep(ANNOUNCE_SPACING).await;
    let mut b = attach("one-identity-on-b", &identity, endpoint, opted_in()).await;
    register_and_announce(&mut b, "b").await;
    assert_ne!(a.destination_hash, b.destination_hash, "two distinct destinations");
    // Precondition: one identity and one interface hash make one tunnel id.
    let a_hash = interface_hash(&a).await;
    assert!(a_hash.is_some(), "a's attached interface is registered");
    assert_eq!(a_hash, interface_hash(&b).await, "both attaches carry one interface hash");
    learn_announce("c learns b", &mut c.announces, &b.destination_hash).await;

    let mut c_out = c.transport.out_link_events();
    let (_link_a, link_to_a) = open_link(&c, &a.destination_hash, "the link c -> a").await;
    match timeout(NO_LINK, a.in_links.recv()).await {
        Err(_elapsed) => {}
        Ok(Ok(event)) => panic!(
            "in-link event for link {} reached a, whose path the reappeared tunnel moved to b's connection",
            event.id
        ),
        Ok(Err(err)) => panic!("a's in-link channel failed: {err:?}"),
    }
    loop {
        match c_out.try_recv() {
            Ok(event) if event.id == link_to_a => assert!(
                !matches!(event.event, LinkEvent::Activated),
                "the third's link to a activated although a's path moved to b"
            ),
            Ok(_) => {}
            Err(TryRecvError::Empty) => break,
            Err(err) => panic!("c's out-link channel failed: {err:?}"),
        }
    }
    // b registered no such destination and drops the request without an in-link event.
    assert_no_in_link_event(&mut b.in_links, "b");

    stop_all(vec![a, b, c], shared).await;
}

/// The usage the two-clients shape implies once both processes are up: two local clients
/// under one identity announce their destinations back to back and a third local client
/// learns both. Python's `LocalClientInterface.should_ingress_limit` returns `False`
/// (`RNS/Interfaces/LocalInterface.py:130`, both for a client program and for the
/// interfaces rnsd spawns per accepted client), and `Interface.incoming_announce_frequency`
/// reads 0 until more than `IC_DEQUE_MIN_SAMPLE` = 2 samples exist (`Interface.py:84`,
/// `:346-348`), so a Python third client files both paths at once. This pins the
/// two-announce usage on a Rust third client: both destinations filed within `WAIT`, and a
/// link to the second activates on the second. The exemption itself is pinned by
/// `shared_child_announce_admission.rs`, the unit test
/// `shared_instance_interface_is_never_ingress_limited` and
/// `tests/announce_admission_usage_probe.rs`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn third_client_admits_two_local_client_announces_back_to_back() {
    let (shared, endpoint) = flagged_shared_instance().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let mut c = attach(
        "one-identity-burst-c",
        &PrivateIdentity::new_from_rand(OsRng),
        endpoint,
        SharedInstanceOptions::default(),
    )
    .await;
    let mut a =
        attach("one-identity-burst-a", &identity, endpoint, SharedInstanceOptions::default()).await;
    let mut b =
        attach("one-identity-burst-b", &identity, endpoint, SharedInstanceOptions::default()).await;

    register_and_announce(&mut a, "a").await;
    tokio::time::sleep(BACK_TO_BACK).await;
    register_and_announce(&mut b, "b").await;
    assert_ne!(a.destination_hash, b.destination_hash, "two distinct destinations");
    let started = Instant::now();
    learn_announce("c learns a", &mut c.announces, &a.destination_hash).await;
    learn_announce(
        "c learns b, announced 100 ms after a (a Python local client holds neither)",
        &mut c.announces,
        &b.destination_hash,
    )
    .await;
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "c learned both destinations after {:?}; a held announce would surface no sooner than 30 s",
        started.elapsed()
    );

    let mut c_out = c.transport.out_link_events();
    let (_link_b, link_to_b) = open_link(&c, &b.destination_hash, "the link c -> b").await;
    await_activated(&mut b.in_links, link_to_b, "the link to b's destination to activate on b")
        .await;
    await_activated(&mut c_out, link_to_b, "the link to b's destination to activate on c").await;
    assert_no_in_link_event(&mut a.in_links, "a");

    stop_all(vec![a, b, c], shared).await;
}
