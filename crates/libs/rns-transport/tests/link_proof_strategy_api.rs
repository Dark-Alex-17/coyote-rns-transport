//! The README's `## Delivery proofs on a link` surface used from outside the crate, the way
//! a downstream application reaches it: `SingleInputDestination::link_proof_strategy` answers
//! `All` until `set_link_proof_strategy` says otherwise; an out-link a consumer opens through
//! `Transport::link` starts out `All` and takes `Link::set_proof_strategy` and
//! `Link::set_proof_requested_callback`; under `App` with no callback the listener posts the
//! plain Data packet as `LinkEvent::Data` whose `LinkPayload::packet_hash` names it and sends
//! no proof until the application calls `Transport::prove_link_packet`, after which the sender
//! sees exactly one `LinkEvent::PacketProven` for that hash; `Link::prove_hash` builds a
//! `Proof` packet addressed to the link; an unknown link id is `InvalidArgument` and a link
//! whose interface is gone is `ConnectionError`, never `InvalidArgument`; a link accepted
//! under `App` asks the callback it copied at accept per packet and keeps it after the
//! destination changes strategy and callback. The in-crate
//! `tests_parts` fixtures pin the same behaviour from inside, where `pub(crate)` items are
//! visible too; this file is the consumer's view, and its last test drives `rustc` the way
//! `breaking_changes_compile_fail.rs` does to pin that every new proof item the README names
//! resolves downstream and that `LinkPayload::with_packet_hash` is not part of that view.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::{Link, LinkEvent, LinkEventData, LinkPayload};
use rns_transport::destination::{
    DestinationName, ProofRequestedHandler, ProofStrategy, SingleInputDestination,
};
use rns_transport::error::RnsError;
use rns_transport::hash::{AddressHash, Hash};
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::tcp_server::{TcpListenerRuntimeStatusHandle, TcpServer};
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{AnnounceEvent, SendPacketOutcome, Transport, TransportConfig};
use rns_transport::{DestinationType, Packet, PacketType};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, Mutex};
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
/// How long a link event channel is watched to show that no proof arrived.
const QUIET: Duration = Duration::from_millis(400);

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
        "link-proof-strategy-api-listener",
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

#[deny(clippy::await_holding_invalid_type)]
async fn connector(port: u16) -> (Transport, broadcast::Receiver<AnnounceEvent>) {
    let transport = Transport::new(TransportConfig::new(
        "link-proof-strategy-api-connector",
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

/// The first event for `link_id` that `pick` accepts.
#[deny(clippy::await_holding_invalid_type)]
async fn await_event<T>(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    what: &str,
    mut pick: impl FnMut(&LinkEvent) -> Option<T>,
) -> T {
    timeout(WAIT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.id == link_id => {
                    if let Some(found) = pick(&event.event) {
                        return found;
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// Sleeps `QUIET`, then drains `events` and returns the `PacketProven` hashes for `link_id`.
#[deny(clippy::await_holding_invalid_type)]
async fn proven_after_quiet(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
) -> Vec<Hash> {
    tokio::time::sleep(QUIET).await;
    let mut seen = Vec::new();
    loop {
        match events.try_recv() {
            Ok(event) => {
                if event.id == link_id {
                    if let LinkEvent::PacketProven { packet_hash } = event.event {
                        seen.push(packet_hash);
                    }
                }
            }
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) => return seen,
            Err(TryRecvError::Closed) => panic!("link event channel closed"),
        }
    }
}

/// A listener whose destination accepts links under `strategy`, a connector that opened a
/// link to it, and both event streams subscribed before the link request went out.
struct ProofPair {
    listener: Listener,
    destination: Arc<Mutex<SingleInputDestination>>,
    connector: Transport,
    link: Arc<Mutex<Link>>,
    link_id: AddressHash,
    in_events: broadcast::Receiver<LinkEventData>,
    out_events: broadcast::Receiver<LinkEventData>,
}

#[deny(clippy::await_holding_invalid_type)]
async fn proof_pair(aspect: &str, strategy: ProofStrategy) -> ProofPair {
    proof_pair_with_callback(aspect, strategy, None).await
}

/// `proof_pair` whose destination also had `callback` set before the link was accepted.
#[deny(clippy::await_holding_invalid_type)]
async fn proof_pair_with_callback(
    aspect: &str,
    strategy: ProofStrategy,
    callback: Option<Box<dyn ProofRequestedHandler>>,
) -> ProofPair {
    let listener = listener().await;
    let (connector, mut announces) = connector(listener.port).await;
    wait_until("the listener to accept", || {
        listener.status.to_json()["accepted_connections"].as_u64() == Some(1)
    })
    .await;
    let mut in_events = listener.transport.in_link_events();
    let mut out_events = connector.out_link_events();

    let destination = listener
        .transport
        .add_destination(
            PrivateIdentity::new_from_rand(OsRng),
            DestinationName::new("example", aspect),
        )
        .await;
    assert_eq!(
        destination.lock().await.link_proof_strategy(),
        ProofStrategy::All,
        "a fresh destination hands its links `All`, the README default"
    );
    destination.lock().await.set_link_proof_strategy(strategy);
    assert_eq!(destination.lock().await.link_proof_strategy(), strategy);
    if let Some(callback) = callback {
        destination.lock().await.set_proof_requested_callback(callback);
    }
    let destination_hash = destination.lock().await.desc.address_hash;
    let announce = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(announce).await;
    learn_announce(&mut announces, &destination_hash).await;

    let desc = connector.destination_desc(&destination_hash).await.expect("the desc was learned");
    let link = connector.link(desc).await.expect("the link opens");
    let link_id = *link.lock().await.id();
    // Both ends must report the link active: the out-link binds its interface on its own
    // `Activated`, which the listener's does not imply.
    await_event(&mut in_events, link_id, "the link to activate on the listener", |event| {
        matches!(event, LinkEvent::Activated).then_some(())
    })
    .await;
    await_event(&mut out_events, link_id, "the link to activate on the connector", |event| {
        matches!(event, LinkEvent::Activated).then_some(())
    })
    .await;
    ProofPair { listener, destination, connector, link, link_id, in_events, out_events }
}

/// Sends `body` as a plain Data packet over the out-link and returns the packet's hash.
#[deny(clippy::await_holding_invalid_type)]
async fn send_plain_data(connector: &Transport, link: &Arc<Mutex<Link>>, body: &[u8]) -> Hash {
    let packet = link.lock().await.data_packet(body).expect("plain data fits a packet");
    let hash = packet.hash();
    let outcome = connector.send_link_packet_on_bound_iface(link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "plain data reaches the wire");
    hash
}

/// The receiving application's half of the README flow, under `App` with no callback: the
/// Data event names the packet through `packet_hash`, nothing is proved until
/// `prove_link_packet`, and then the sender sees exactly one `PacketProven` for that hash.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn app_receiver_defers_the_proof_until_the_application_proves() {
    let mut pair = proof_pair("deferred", ProofStrategy::App).await;
    let link_id = pair.link_id;
    assert_eq!(
        pair.link.lock().await.proof_strategy(),
        ProofStrategy::All,
        "an out-link, which no destination accepts, starts out `All`"
    );

    let sent = send_plain_data(&pair.connector, &pair.link, b"deferred").await;
    let payload: LinkPayload =
        await_event(&mut pair.in_events, link_id, "the plain Data event", |event| match event {
            LinkEvent::Data(payload) if payload.as_slice() == b"deferred" => {
                Some((**payload).clone())
            }
            _ => None,
        })
        .await;
    assert_eq!(
        payload.packet_hash(),
        Some(sent),
        "the posted payload names the packet it came from"
    );
    assert!(
        proven_after_quiet(&mut pair.out_events, link_id).await.is_empty(),
        "under `App` with no callback the sender sees no PacketProven on its own"
    );

    pair.listener
        .transport
        .prove_link_packet(&link_id, &sent)
        .await
        .expect("the application's proof goes out on the link's interface");
    let proven =
        await_event(
            &mut pair.out_events,
            link_id,
            "the sender's PacketProven",
            |event| match event {
                LinkEvent::PacketProven { packet_hash } => Some(*packet_hash),
                _ => None,
            },
        )
        .await;
    assert_eq!(proven, sent, "the sender's PacketProven names the packet the application proved");
    assert!(
        proven_after_quiet(&mut pair.out_events, link_id).await.is_empty(),
        "exactly one PacketProven: the deferred proof is not repeated"
    );

    timeout(WAIT, pair.connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, pair.listener.transport.stop()).await.expect("listener stop completes");
}

/// The out-link half of the README section: a consumer sets the strategy and the callback on
/// the link it holds, `prove_hash` builds a `Proof` link packet addressed to the link, and a
/// payload the consumer builds itself names no packet.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn out_link_takes_the_strategy_and_the_callback_and_prove_hash_addresses_the_link() {
    let pair = proof_pair("out-link", ProofStrategy::All).await;
    {
        let mut link = pair.link.lock().await;
        link.set_proof_strategy(ProofStrategy::None);
        assert_eq!(link.proof_strategy(), ProofStrategy::None);
        link.set_proof_strategy(ProofStrategy::App);
        link.set_proof_requested_callback(Box::new(|_: &Packet| true));
        assert_eq!(link.proof_strategy(), ProofStrategy::App);

        let hash = Hash::new([0x42; 32]);
        let proof = link.prove_hash(&hash);
        assert_eq!(proof.header.packet_type, PacketType::Proof);
        assert_eq!(proof.header.destination_type, DestinationType::Link);
        assert_eq!(proof.destination, *link.id(), "the proof is addressed to the link");
        assert!(
            proof.data.as_slice().starts_with(&hash.to_bytes()),
            "the proof carries the proved hash first"
        );
    }
    assert_eq!(
        LinkPayload::new_from_slice(b"mine").packet_hash(),
        None,
        "a payload the consumer builds was decrypted from no packet"
    );

    timeout(WAIT, pair.connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, pair.listener.transport.stop()).await.expect("listener stop completes");
}

/// The two refusals the rustdoc promises, told apart: a link id this transport never saw is
/// `InvalidArgument`; a link it still holds whose peer's interface is gone is
/// `ConnectionError`, and the gone interface never turns into `InvalidArgument`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn prove_link_packet_tells_an_unknown_link_from_a_gone_interface() {
    let mut pair = proof_pair("refusals", ProofStrategy::App).await;
    let link_id = pair.link_id;
    let sent = send_plain_data(&pair.connector, &pair.link, b"orphan").await;
    await_event(&mut pair.in_events, link_id, "the plain Data event", |event| {
        matches!(event, LinkEvent::Data(payload) if payload.as_slice() == b"orphan").then_some(())
    })
    .await;

    let unknown = AddressHash::new_from_rand(OsRng);
    assert!(
        matches!(
            pair.listener.transport.prove_link_packet(&unknown, &sent).await,
            Err(RnsError::InvalidArgument)
        ),
        "a link id this transport never saw"
    );

    // The peer goes away; the listener still holds the in-link, but its interface takes
    // nothing any more.
    timeout(WAIT, pair.connector.stop()).await.expect("connector stop completes");
    let deadline = Instant::now() + WAIT;
    loop {
        match pair.listener.transport.prove_link_packet(&link_id, &sent).await {
            Err(RnsError::ConnectionError) => break,
            Ok(()) => {
                assert!(
                    Instant::now() < deadline,
                    "the proof kept finding an interface after the peer stopped"
                );
                tokio::time::sleep(POLL).await;
            }
            other => {
                panic!("a held link whose interface is gone is ConnectionError, got {other:?}")
            }
        }
    }

    timeout(WAIT, pair.listener.transport.stop()).await.expect("listener stop completes");
}

/// A callback that answers `verdict` and counts how often the link asked it.
fn counting_callback(verdict: bool) -> (Box<dyn ProofRequestedHandler>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    (
        Box::new(move |_: &Packet| {
            counter.fetch_add(1, Ordering::SeqCst);
            verdict
        }),
        calls,
    )
}

/// The divergence register's "copied onto a link when it is accepted" sentence, callback
/// half: an in-link accepted under `App` with a refusing destination callback keeps asking
/// that callback after the destination is switched to `All` and handed a consenting one;
/// neither change reaches the link, the consumer reads the link's own `App` back, and the
/// deferred `prove_link_packet` still closes the loop for the sender.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accepted_link_keeps_the_callback_it_was_handed_at_accept() {
    let (refusing, refusals) = counting_callback(false);
    let mut pair =
        proof_pair_with_callback("callback-snapshot", ProofStrategy::App, Some(refusing)).await;
    let link_id = pair.link_id;

    let first = send_plain_data(&pair.connector, &pair.link, b"ask-me").await;
    await_event(&mut pair.in_events, link_id, "the first plain Data event", |event| {
        matches!(event, LinkEvent::Data(payload) if payload.as_slice() == b"ask-me").then_some(())
    })
    .await;
    assert!(
        proven_after_quiet(&mut pair.out_events, link_id).await.is_empty(),
        "a refusing callback leaves the packet unproved"
    );
    assert_eq!(refusals.load(Ordering::SeqCst), 1, "the copied callback was asked once");

    // Change both halves on the destination after the accept.
    let (consenting, consents) = counting_callback(true);
    let in_link = pair
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("the listener holds the accepted in-link");
    assert_eq!(
        in_link.lock().await.proof_strategy(),
        ProofStrategy::App,
        "the accepted link reads back the strategy it was handed"
    );
    {
        let mut destination = pair.destination.lock().await;
        destination.set_link_proof_strategy(ProofStrategy::All);
        destination.set_proof_requested_callback(consenting);
        assert_eq!(destination.link_proof_strategy(), ProofStrategy::All);
    }

    let second = send_plain_data(&pair.connector, &pair.link, b"ask-me-again").await;
    await_event(&mut pair.in_events, link_id, "the second plain Data event", |event| {
        matches!(event, LinkEvent::Data(payload) if payload.as_slice() == b"ask-me-again")
            .then_some(())
    })
    .await;
    assert!(
        proven_after_quiet(&mut pair.out_events, link_id).await.is_empty(),
        "neither the destination's new strategy nor its new callback proves on the old link"
    );
    assert_eq!(refusals.load(Ordering::SeqCst), 2, "the link still asks the callback it copied");
    assert_eq!(consents.load(Ordering::SeqCst), 0, "the destination's new callback is not reached");
    assert_eq!(
        in_link.lock().await.proof_strategy(),
        ProofStrategy::App,
        "the destination's switch to `All` did not reach the accepted link"
    );
    assert_ne!(first, second, "two distinct packets were sent");

    pair.listener
        .transport
        .prove_link_packet(&link_id, &second)
        .await
        .expect("the deferred proof still goes out");
    let proven =
        await_event(
            &mut pair.out_events,
            link_id,
            "the sender's PacketProven",
            |event| match event {
                LinkEvent::PacketProven { packet_hash } => Some(*packet_hash),
                _ => None,
            },
        )
        .await;
    assert_eq!(proven, second);

    timeout(WAIT, pair.connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, pair.listener.transport.stop()).await.expect("listener stop completes");
}

/// The `None` arm of the README section seen from the receiving side: a destination that
/// hands its links `None` (and, to tell `None` from `App`, a consenting callback) accepts an
/// in-link that reads `None`, never asks the callback, never proves a plain Data packet on
/// its own, and still lets `prove_link_packet` close the loop for the sender exactly once.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn none_on_the_destination_reaches_the_accepted_in_link_and_only_the_app_proves() {
    let (consenting, consents) = counting_callback(true);
    let mut pair =
        proof_pair_with_callback("in-link-none", ProofStrategy::None, Some(consenting)).await;
    let link_id = pair.link_id;
    let in_link = pair
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("the listener tracks the accepted in-link");
    assert_eq!(
        in_link.lock().await.proof_strategy(),
        ProofStrategy::None,
        "the destination's `None` is copied onto the accepted in-link"
    );
    assert_eq!(
        pair.link.lock().await.proof_strategy(),
        ProofStrategy::All,
        "the out-link, which no destination accepts, keeps the `All` default"
    );

    let sent = send_plain_data(&pair.connector, &pair.link, b"in-link-none").await;
    let payload: LinkPayload =
        await_event(&mut pair.in_events, link_id, "the plain Data event", |event| match event {
            LinkEvent::Data(payload) if payload.as_slice() == b"in-link-none" => {
                Some((**payload).clone())
            }
            _ => None,
        })
        .await;
    assert_eq!(payload.packet_hash(), Some(sent), "the posted payload names the packet");
    assert!(
        proven_after_quiet(&mut pair.out_events, link_id).await.is_empty(),
        "under `None` the in-link proves nothing on its own"
    );
    assert_eq!(
        consents.load(Ordering::SeqCst),
        0,
        "`None` never consults the callback; a consenting one changes nothing"
    );

    pair.listener
        .transport
        .prove_link_packet(&link_id, &sent)
        .await
        .expect("the application's proof goes out on the link's interface");
    assert_eq!(
        proven_after_quiet(&mut pair.out_events, link_id).await,
        vec![sent],
        "exactly one PacketProven, naming the packet the application proved"
    );

    timeout(WAIT, pair.connector.stop()).await.expect("connector stop completes");
    timeout(WAIT, pair.listener.transport.stop()).await.expect("listener stop completes");
}

/// What a downstream crate sees of the surface at compile time: every new proof item the
/// README names resolves, and `LinkPayload::with_packet_hash`, which only the link may call,
/// does not.
const CONSUMER: &str = r#"
use rns_transport::destination::link::{Link, LinkPayload};
use rns_transport::destination::{ProofStrategy, SingleInputDestination};
use rns_transport::hash::{AddressHash, Hash};
use rns_transport::transport::Transport;
use rns_transport::Packet;

pub fn destination_default(destination: &mut SingleInputDestination) -> ProofStrategy {
    destination.set_link_proof_strategy(ProofStrategy::App);
    destination.set_proof_requested_callback(Box::new(|_: &Packet| true));
    destination.link_proof_strategy()
}

pub fn link_policy(link: &mut Link) -> Packet {
    link.set_proof_strategy(ProofStrategy::None);
    link.set_proof_requested_callback(Box::new(|_: &Packet| false));
    let _ = link.proof_strategy();
    link.prove_hash(&Hash::new([0u8; 32]))
}

pub fn payload_hash(payload: &LinkPayload) -> Option<Hash> {
    payload.packet_hash()
}

pub async fn prove(transport: &Transport, link_id: &AddressHash, hash: &Hash) -> bool {
    transport.prove_link_packet(link_id, hash).await.is_ok()
}
"#;

const REJECTED: &str = "use rns_transport::destination::link::LinkPayload;\n\
     use rns_transport::hash::Hash;\n\
     pub fn forge(hash: Hash) -> LinkPayload { LinkPayload::new().with_packet_hash(hash) }\n";
const REJECTED_DIAGNOSTIC: &str = "method `with_packet_hash` is private";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir()
            .join(format!("rns-transport-link-proof-strategy-api-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("scratch directory should be creatable");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn deps_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary should know its own path");
    exe.parent().map(Path::to_path_buf).expect("test binary should live in target/<profile>/deps")
}

fn candidate_rlibs(deps: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(deps)
        .expect("deps directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                name.starts_with("librns_transport-") && name.ends_with(".rlib")
            })
        })
        .filter_map(|path| {
            let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
            Some((modified, path))
        })
        .collect();
    found.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    found.into_iter().map(|(_, path)| path).collect()
}

fn rustc(
    scratch: &Path,
    deps: &Path,
    rlib: &Path,
    name: &str,
    source: &str,
) -> std::process::Output {
    let file = scratch.join(format!("{name}.rs"));
    fs::write(&file, source).expect("snippet source should be writable");
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    Command::new(compiler)
        .args(["--edition", "2021", "--crate-type", "lib", "--crate-name", name])
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("rns_transport={}", rlib.display()))
        .arg("--out-dir")
        .arg(scratch)
        .arg(&file)
        .output()
        .expect("rustc should be runnable from a cargo test")
}

#[test]
fn readme_items_resolve_downstream_and_with_packet_hash_does_not() {
    let scratch = Scratch::new();
    let deps = deps_dir();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib next to {}", deps.display());
    let mut failures = Vec::new();
    let mut rlib = None;
    for candidate in candidates {
        let output = rustc(&scratch.0, &deps, &candidate, "proof_consumer", CONSUMER);
        if output.status.success() {
            rlib = Some(candidate);
            break;
        }
        failures.push(format!(
            "{}:\n{}",
            candidate.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let rlib = rlib.unwrap_or_else(|| {
        panic!(
            "the README's proof items should resolve from a downstream crate:\n{}",
            failures.join("\n")
        )
    });

    let output = rustc(&scratch.0, &deps, &rlib, "proof_forger", REJECTED);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "with_packet_hash compiled downstream; only the link may set it"
    );
    assert!(stderr.contains(REJECTED_DIAGNOSTIC), "not the promised diagnostic:\n{stderr}");
}
