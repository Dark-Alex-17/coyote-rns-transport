//! The README's `destination_desc` snippet, verbatim, run from outside the crate: a
//! listener with a `TcpServer` on 127.0.0.1 announces `example/echo`, a connector with a
//! `TcpClient` learns the announce, resolves the desc, and opens a link with it. Around
//! the snippet, the claims a first-time consumer reads next: `destination_identity` is
//! `Some` for the same address, and the link the desc opens activates on the announcing
//! side under the same id. The cold-start path runs the same snippet before the announce
//! has been learned: it stops at its own guard with no link and `destination_identity`
//! `None`, and the very same transport then resolves the same address once the announce
//! arrives, so a miss is not remembered. `resolve_destination` is exercised the same way
//! from outside the crate: an announced destination answers at once, one registered without
//! announcing is learned through the path request, and a zero timeout never asks the network.
//! The two desc/path-table splits the README names are walked as a consumer would: an
//! expired path answers `Some` at once and `await_path` relearns the next hop, a deregistered
//! destination with a standing path answers `None` at once and `expire_path` lets it relearn.

use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{Link, LinkEvent, LinkEventData};
use rns_transport::destination::DestinationName;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, Transport, TransportConfig};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// How long a link event channel is watched to show that nothing arrived.
const QUIET: Duration = Duration::from_millis(300);
/// The headroom a bounded miss may run past its budget before the test calls it late;
/// generous so a loaded CI host does not fail the bound.
const MISS_SLACK: Duration = Duration::from_secs(2);
/// The README snippet's own message for a desc that is not there yet.
const NOT_LEARNED: &str = "the announce has not been learned yet";

/// The announcing side: a transport with a `TcpServer` on 127.0.0.1.
struct Listener {
    transport: Transport,
    port: u16,
    status: TcpListenerRuntimeStatusHandle,
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
async fn free_loopback_port() -> u16 {
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

#[deny(clippy::await_holding_invalid_type)]
async fn listener() -> Listener {
    let transport = Transport::new(TransportConfig::new(
        "readme-destination-desc-listener",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let port = free_loopback_port().await;
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the listener to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    Listener { transport, port, status }
}

/// The connecting side: a transport with a `TcpClient` on the listener's port. The
/// announce receiver is subscribed before the stream is up so the announce cannot be missed.
#[deny(clippy::await_holding_invalid_type)]
async fn connector(port: u16) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport = Transport::new(TransportConfig::new(
        "readme-destination-desc-connector",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let announces = transport.recv_announces().await;
    let client = TcpClient::new(format!("127.0.0.1:{port}"));
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
async fn learn_announce(announces: &mut broadcast::Receiver<AnnounceEvent>, hash: &AddressHash) {
    timeout(WAIT, async {
        loop {
            match announces.recv().await {
                Ok(event) => {
                    if event.destination.lock().await.desc.address_hash == *hash {
                        return;
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("announce channel closed"),
            }
        }
    })
    .await
    .expect("the announce arrives")
}

#[deny(clippy::await_holding_invalid_type)]
async fn path_response_announces_for(
    announces: &mut broadcast::Receiver<AnnounceEvent>,
    hash: &AddressHash,
    window: Duration,
) -> usize {
    let deadline = Instant::now() + window;
    let mut seen = 0;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return seen;
        }
        match timeout(left, announces.recv()).await {
            Ok(Ok(event)) => {
                if event.destination.lock().await.desc.address_hash == *hash {
                    assert!(event.is_path_response, "only a path response is expected here");
                    seen += 1;
                }
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) => panic!("announce channel closed"),
            Err(_) => return seen,
        }
    }
}

#[deny(clippy::await_holding_invalid_type)]
async fn await_activated(events: &mut broadcast::Receiver<LinkEventData>) -> LinkEventData {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if matches!(event.event, LinkEvent::Activated) => return event,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .expect("the link activates on the announcing side")
}

/// The README `destination_desc` snippet, verbatim.
#[deny(clippy::await_holding_invalid_type)]
async fn readme_destination_desc_and_link(
    transport: &Transport,
    destination_hash: AddressHash,
) -> Result<Arc<Mutex<Link>>, Box<dyn Error + Send + Sync>> {
    let desc = transport
        .destination_desc(&destination_hash)
        .await
        .ok_or("the announce has not been learned yet")?;
    assert_eq!(
        desc.name.as_name_hash_slice(),
        DestinationName::new("example", "echo").as_name_hash_slice(),
    );
    let link = transport.link(desc).await?;
    Ok(link)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippet_resolves_the_desc_and_opens_a_link() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    // The accepted child interface that carries the listener's traffic is registered a
    // task wake after the connector reports its stream connected.
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let mut in_events = listener.transport.in_link_events();

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &destination_hash).await;

    let link = readme_destination_desc_and_link(&connector, destination_hash)
        .await
        .expect("the README snippet");
    assert!(
        connector.destination_identity(&destination_hash).await.is_some(),
        "destination_identity is Some whenever destination_desc is Some"
    );
    let link_id = *link.lock().await.id();
    let activated = await_activated(&mut in_events).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

/// The README's first split, walked as a consumer: once the path table entry is gone but
/// the desc is still held, `resolve_destination` answers `Some` at once and asks the network
/// nothing (no path response arrives), so the caller follows the README and calls
/// `await_path` itself, which sends the one path request, draws exactly one path response,
/// restores `has_path`, and the desc then opens a link that activates on the announcing side.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_split_expired_path_answers_at_once_and_await_path_relearns_the_next_hop() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let mut in_events = listener.transport.in_link_events();

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &destination_hash).await;
    assert!(connector.has_path(&destination_hash).await, "the announce filled the path table");

    assert!(connector.expire_path(&destination_hash).await, "the path table entry is removed");
    assert!(!connector.has_path(&destination_hash).await);
    assert!(
        connector.destination_desc(&destination_hash).await.is_some(),
        "expiring the path leaves the learned desc standing"
    );

    let budget = Duration::from_secs(1);
    let started = Instant::now();
    let resolved = connector
        .resolve_destination(&destination_hash, budget)
        .await
        .expect("the held desc answers although the path is gone");
    assert_eq!(resolved.address_hash, destination_hash);
    assert!(
        started.elapsed() < budget,
        "a held desc answers at once, not after the timeout, took {:?}",
        started.elapsed()
    );
    assert_eq!(
        path_response_announces_for(&mut announces, &destination_hash, QUIET).await,
        0,
        "a held desc sends no path request, so no path response arrives"
    );
    assert!(!connector.has_path(&destination_hash).await, "the lookup did not refill the path");

    // The README's next step for a caller that needs a next hop.
    assert!(
        connector.await_path(&destination_hash, WAIT, None).await,
        "await_path relearns the path through its own request"
    );
    assert!(connector.has_path(&destination_hash).await);
    assert_eq!(
        path_response_announces_for(&mut announces, &destination_hash, QUIET).await,
        1,
        "exactly one path response answers the one request await_path sent"
    );

    let link = readme_destination_desc_and_link(&connector, destination_hash)
        .await
        .expect("the relearned path carries the link request");
    let link_id = *link.lock().await.id();
    let activated = await_activated(&mut in_events).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

/// The README's second split, walked as a consumer: a destination deregistered while its
/// path table entry stands answers `None` at once and asks the network nothing, and the
/// miss is final for as long as the path stands. Expiring the stale path turns the address
/// back into the unknown-but-reachable case: the next call sends the one path request,
/// draws exactly one path response, and answers `Some` again.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_split_deregistered_with_a_standing_path_answers_none_until_the_path_expires() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &destination_hash).await;

    assert!(connector.deregister_destination(&destination_hash).await, "the desc is removed");
    assert!(connector.destination_desc(&destination_hash).await.is_none());
    assert!(connector.has_path(&destination_hash).await, "the path table entry still stands");

    let budget = Duration::from_secs(2);
    for attempt in 1..=2u8 {
        let started = Instant::now();
        let missed = connector.resolve_destination(&destination_hash, budget).await;
        assert!(missed.is_none(), "attempt {attempt}: the entry is gone and nothing refills it");
        assert!(
            started.elapsed() < budget,
            "attempt {attempt}: the standing path answers at once, took {:?}",
            started.elapsed()
        );
    }
    assert_eq!(
        path_response_announces_for(&mut announces, &destination_hash, QUIET).await,
        0,
        "no path request goes out while the path stands, so no path response arrives"
    );

    assert!(connector.expire_path(&destination_hash).await, "the stale path is removed");
    let relearned = connector
        .resolve_destination(&destination_hash, WAIT)
        .await
        .expect("without the stale path the one path request draws the announce");
    assert_eq!(relearned.address_hash, destination_hash);
    assert!(connector.has_path(&destination_hash).await, "the path response refilled the path");
    assert_eq!(
        path_response_announces_for(&mut announces, &destination_hash, QUIET).await,
        1,
        "exactly one path response answers the one request the call sent"
    );

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippet_before_the_announce_names_the_miss_and_resolves_once_learned() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let mut in_events = listener.transport.in_link_events();

    // The destination exists on the announcing side but has not announced yet, so the
    // connector has nothing to resolve: the snippet stops at its own guard, opens no link,
    // and `destination_identity` says the same.
    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let destination_hash = destination.lock().await.desc.address_hash;

    let miss = readme_destination_desc_and_link(&connector, destination_hash)
        .await
        .err()
        .expect("the snippet fails before the announce is learned");
    assert_eq!(miss.to_string(), NOT_LEARNED, "the snippet's guard names the miss");
    assert!(
        connector.destination_identity(&destination_hash).await.is_none(),
        "destination_identity is None whenever destination_desc is None"
    );
    assert!(
        timeout(QUIET, in_events.recv()).await.is_err(),
        "no link reaches the announcing side while the desc is unknown"
    );

    // The same transport resolves the same address once the announce arrives: a miss is
    // a lookup result, not a remembered answer.
    let packet = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &destination_hash).await;

    let link = readme_destination_desc_and_link(&connector, destination_hash)
        .await
        .expect("the README snippet succeeds once the announce is learned");
    let link_id = *link.lock().await.id();
    let activated = await_activated(&mut in_events).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_answers_known_and_reachable_destinations() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let announced = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "echo"),
        )
        .await;
    let announced_hash = announced.lock().await.desc.address_hash;
    let packet = announced.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(packet).await;
    learn_announce(&mut announces, &announced_hash).await;

    let known =
        timeout(Duration::from_secs(1), connector.resolve_destination(&announced_hash, WAIT))
            .await
            .expect("a learned destination answers without waiting")
            .expect("the announce was learned");
    assert_eq!(known.address_hash, announced_hash);

    // Registered on the announcing side but never announced: only the path request the
    // call sends draws the announce that fills the desc.
    let silent = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "quiet"),
        )
        .await;
    let silent_hash = silent.lock().await.desc.address_hash;
    assert!(connector.destination_desc(&silent_hash).await.is_none());

    let learned = connector
        .resolve_destination(&silent_hash, WAIT)
        .await
        .expect("the path request drew the announce");
    assert_eq!(learned.address_hash, silent_hash);
    assert!(connector.destination_identity(&silent_hash).await.is_some());

    let unknown = AddressHash::new_from_rand(OsRng);
    assert!(
        connector.resolve_destination(&unknown, Duration::ZERO).await.is_none(),
        "a zero timeout is a cached lookup alone"
    );

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}

/// The one path request is attempted, not guaranteed a wire: a transport with no interface
/// at all has nothing to broadcast on, and `resolve_destination` still keeps its contract
/// from outside the crate. The unknown destination misses no sooner than the timeout and
/// within the slack of it, a zero timeout answers `None` at once, and the
/// transport stops cleanly afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_without_an_interface_misses_on_the_timeout() {
    let isolated = Transport::new(TransportConfig::new(
        "readme-destination-desc-isolated",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    ));
    let unknown = AddressHash::new_from_rand(OsRng);

    let started = Instant::now();
    assert!(
        isolated.resolve_destination(&unknown, Duration::ZERO).await.is_none(),
        "a zero timeout is a cached lookup alone"
    );
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "a zero timeout never waits a poll, took {:?}",
        started.elapsed()
    );

    let budget = Duration::from_millis(300);
    let started = Instant::now();
    let missed = isolated.resolve_destination(&unknown, budget).await;
    let elapsed = started.elapsed();
    assert!(missed.is_none(), "nothing can answer without an interface");
    assert!(elapsed >= budget, "the call waits out the budget, waited {elapsed:?}");
    assert!(
        elapsed < budget + MISS_SLACK,
        "the call returns within the slack of the budget, waited {elapsed:?}"
    );
    assert!(!isolated.has_path(&unknown).await, "a miss leaves no path behind");
    assert!(isolated.destination_desc(&unknown).await.is_none(), "a miss leaves no desc behind");

    timeout(WAIT, isolated.stop()).await.expect("isolated stop completes");
}

/// From outside the crate, a miss is bounded by the caller's timeout: the unreachable arm
/// returns `None` no sooner than the timeout and within the slack of it, so a
/// caller that budgets `timeout` for the lookup is not held past it. A fresh identity
/// resolves the same way afterwards, so a miss is not remembered against the transport.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resolve_destination_miss_is_bounded_by_the_timeout_and_not_remembered() {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;

    let unknown = AddressHash::new_from_rand(OsRng);
    let budget = Duration::from_millis(400);
    let started = Instant::now();
    let missed = connector.resolve_destination(&unknown, budget).await;
    let elapsed = started.elapsed();
    assert!(missed.is_none(), "nobody holds the destination");
    assert!(elapsed >= budget, "the call waits out the budget, waited {elapsed:?}");
    assert!(
        elapsed < budget + MISS_SLACK,
        "the call returns within the slack of the budget, waited {elapsed:?}"
    );

    let quiet = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", "quiet-after-miss"),
        )
        .await;
    let quiet_hash = quiet.lock().await.desc.address_hash;
    let learned = connector
        .resolve_destination(&quiet_hash, WAIT)
        .await
        .expect("the miss before it does not stop the next lookup");
    assert_eq!(learned.address_hash, quiet_hash);
    learn_announce(&mut announces, &quiet_hash).await;

    timeout(WAIT, connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, listener.transport.stop()).await.expect("listener stop completes");
}
