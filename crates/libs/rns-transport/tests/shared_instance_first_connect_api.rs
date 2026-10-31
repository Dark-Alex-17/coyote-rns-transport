//! The README's "A client attached while nothing listens also reports each
//! failed attempt" paragraph, followed from outside the crate against an
//! in-process shared instance, the way `tests/shared_instance_api.rs` follows
//! the attach snippets. The in-crate pins
//! (`two_transport_shared_instance_dial_failures.rs`) hold the same promises
//! through crate-private helpers; a downstream consumer only has the public
//! surface, so these pins use nothing else:
//!
//! - by default the attach "returns as soon as the client is spawned and
//!   keeps dialing": against a port nothing listens on it returns `Ok` at
//!   once, the stream reads `Connecting` then `DialFailed { Refused }` per
//!   attempt, the slot and the posture are set, a `TcpServer` bound on the
//!   port turns the stream to `Connected` with no `DialFailed` after it, and
//!   the detach ends the stream;
//! - `with_first_connect_timeout` against nothing "detaches again and returns
//!   `RnsError::SharedInstanceConnectTimeout`, with `shared_instance()` back
//!   to `None`, the posture off, and the stream reading `Connecting`, zero or
//!   more `DialFailed`, then ending"; a detach after that finds nothing, and
//!   the same transport can attach again to a shared instance that does
//!   listen, the timed-out attach having left nothing behind;
//! - the cleanup of an attach that gave up releases the slot only while the
//!   slot still holds its own interface, so a detach and a second attach
//!   taken during the wait survive it.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::error::RnsError;
use rns_transport::identity::PrivateIdentity;
use rns_transport::iface::tcp_server::TcpServer;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    DialFailureKind, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport,
    TransportConfig,
};
use tokio::sync::mpsc;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(20);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// Several reconnect waits: the quiet window an absence check needs.
const QUIET: Duration = Duration::from_millis(400);

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

fn client_transport(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}

/// A transport-enabled node listening with a `TcpServer` on `port`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_on(port: u16) -> Transport {
    let mut config = TransportConfig::new(
        "probe-shared-instance",
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    );
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let server = TcpServer::new(format!("127.0.0.1:{port}"), transport.iface_manager());
    let status = server.runtime_status_handle();
    transport.iface_manager().lock().await.spawn(server, TcpServer::spawn);
    wait_until("the shared instance to listen", || {
        status.to_json()["listener_state"].as_str() == Some("listening")
    })
    .await;
    transport
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.expect("a state event arrives in time")
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_default_attach_returns_at_once_reports_refusals_and_connects_when_the_server_appears(
) {
    let port = free_loopback_port().await;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let transport = client_transport("probe-dial-failures-client");
    let (events_tx, mut events) = mpsc::channel(16);

    let started = Instant::now();
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx),
        )
        .await
        .expect("the default attach does not wait for a connect");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "the attach returned as soon as the client was spawned: {:?}",
        started.elapsed()
    );
    assert_eq!(transport.shared_instance(), Some(iface), "the slot names the attached iface");
    assert!(transport.connected_to_shared_instance().await, "the posture is set by the attach");

    assert_eq!(next_event(&mut events).await, Some(SharedInstanceEvent::Connecting));
    for attempt in 1..=3 {
        assert_eq!(
            next_event(&mut events).await,
            Some(SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused }),
            "attempt {attempt}: one Refused per failed dial while nothing listens"
        );
    }
    assert!(transport.connected_to_shared_instance().await, "the posture survives the failures");
    assert_eq!(transport.shared_instance(), Some(iface), "the slot survives the failures");

    let shared = shared_instance_on(port).await;
    loop {
        match next_event(&mut events).await {
            Some(SharedInstanceEvent::Connected) => break,
            Some(SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused }) => continue,
            other => {
                panic!("expected Refused or Connected once the server appeared, got {other:?}")
            }
        }
    }
    match timeout(QUIET, events.recv()).await {
        Err(_) => {}
        Ok(other) => panic!("no event may follow Connected while the server stays up: {other:?}"),
    }

    assert!(transport.detach_shared_instance().await, "the detach releases the iface");
    assert_eq!(
        timeout(WAIT, events.recv()).await.expect("the stream ends after the detach"),
        None,
        "the detach ends the stream without a Disconnected"
    );

    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn first_connect_timeout_leaves_nothing_behind_and_the_transport_attaches_again() {
    let port = free_loopback_port().await;
    let endpoint = SocketAddr::from(([127, 0, 0, 1], port));
    let transport = client_transport("probe-first-connect-timeout-client");
    let (events_tx, mut events) = mpsc::channel(16);

    let started = Instant::now();
    let outcome = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx)
                .with_first_connect_timeout(Duration::from_millis(300)),
        )
        .await;
    let elapsed = started.elapsed();
    assert!(
        matches!(outcome, Err(RnsError::SharedInstanceConnectTimeout)),
        "the attach gives up on the first connect: {outcome:?}"
    );
    assert!(
        elapsed >= Duration::from_millis(300) && elapsed < Duration::from_secs(5),
        "the attach waited about the timeout, not less and not forever: {elapsed:?}"
    );
    assert_eq!(transport.shared_instance(), None, "the slot is released");
    assert!(!transport.connected_to_shared_instance().await, "the posture is off");
    assert!(!transport.detach_shared_instance().await, "a detach after the timeout finds nothing");

    // The stream: Connecting, zero or more Refused, then the end, never Connected.
    let mut read = Vec::new();
    while let Some(event) = next_event(&mut events).await {
        read.push(event);
    }
    assert_eq!(read.first(), Some(&SharedInstanceEvent::Connecting), "{read:?}");
    assert!(
        read[1..]
            .iter()
            .all(|event| *event
                == SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused }),
        "only refused dials follow Connecting: {read:?}"
    );
    assert!(
        !read.contains(&SharedInstanceEvent::Connected),
        "nothing listened, so nothing connected: {read:?}"
    );

    // The same transport attaches again, this time to a shared instance that listens.
    let shared = shared_instance_on(port).await;
    let (events_tx, mut events) = mpsc::channel(16);
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx)
                .with_first_connect_timeout(Duration::from_secs(5)),
        )
        .await
        .expect("a second attach after the timed-out one succeeds against a listening server");
    assert_eq!(transport.shared_instance(), Some(iface), "the slot names the new iface");
    assert!(transport.connected_to_shared_instance().await, "the posture is set again");
    assert_eq!(next_event(&mut events).await, Some(SharedInstanceEvent::Connecting));
    assert_eq!(
        next_event(&mut events).await,
        Some(SharedInstanceEvent::Connected),
        "with the timeout satisfied the stream reads Connected"
    );
    assert!(
        transport.interface_hashes().await.contains(&iface),
        "the second attach's iface is registered"
    );

    assert!(transport.detach_shared_instance().await);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
}

/// A detach and a second attach, to a shared instance that listens, taken while a
/// first attach to a dead port still waits on its 1 s first-connect timeout. The slot
/// refuses a second attach while the first holds it, so the detach has to free it
/// first; that detach also stops the first client, whose consumer ends and wakes the
/// waiting attach ahead of its timer. Either way the first attach answers
/// `SharedInstanceConnectTimeout` and leaves the slot alone, since it no longer holds
/// its interface: the second attach stays `shared_instance()`, the posture stays on,
/// and its stream reads `Connecting`, `Connected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn first_connect_cleanup_leaves_a_replacement_attach_in_place() {
    let dead_port = free_loopback_port().await;
    let live_port = free_loopback_port().await;
    let shared = shared_instance_on(live_port).await;
    let transport = client_transport("first-connect-replaced-client");
    let (first_tx, mut first_events) = mpsc::channel(16);
    let (second_tx, mut second_events) = mpsc::channel(16);

    let first = transport.attach_shared_instance(
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], dead_port))),
        SharedInstanceOptions::default()
            .with_reconnect_wait(RECONNECT_WAIT)
            .with_state_events(first_tx)
            .with_first_connect_timeout(Duration::from_secs(1)),
    );
    let replace = async {
        assert_eq!(
            next_event(&mut first_events).await,
            Some(SharedInstanceEvent::Connecting),
            "the first attach holds the slot once its Connecting is out"
        );
        assert!(transport.detach_shared_instance().await, "the detach takes the first attach");
        transport
            .attach_shared_instance(
                SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], live_port))),
                SharedInstanceOptions::default()
                    .with_reconnect_wait(RECONNECT_WAIT)
                    .with_state_events(second_tx),
            )
            .await
            .expect("the second attach finds the slot freed by the detach")
    };
    let (first, second) = tokio::join!(first, replace);
    assert!(
        matches!(first, Err(RnsError::SharedInstanceConnectTimeout)),
        "the first attach never saw a connect: {first:?}"
    );
    assert_eq!(
        transport.shared_instance(),
        Some(second),
        "the first attach's cleanup leaves the replacement in the slot"
    );
    assert!(transport.connected_to_shared_instance().await, "the posture is the replacement's");
    assert_eq!(next_event(&mut second_events).await, Some(SharedInstanceEvent::Connecting));
    assert_eq!(
        next_event(&mut second_events).await,
        Some(SharedInstanceEvent::Connected),
        "the replacement connects to the listening shared instance"
    );

    let mut first_rest = Vec::new();
    while let Some(event) = next_event(&mut first_events).await {
        first_rest.push(event);
    }
    assert!(
        first_rest.iter().all(|event| matches!(event, SharedInstanceEvent::DialFailed { .. })),
        "the first stream reads only refusals before it ends: {first_rest:?}"
    );

    assert!(transport.detach_shared_instance().await, "the replacement is still attached");
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
}

/// A consumer that opts into the timeout but attaches to a live shared
/// instance sees the attach wait only until the connect, not the full
/// timeout.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn first_connect_timeout_returns_on_the_connect_not_on_the_timer() {
    let port = free_loopback_port().await;
    let shared = shared_instance_on(port).await;
    let transport = client_transport("probe-first-connect-ok-client");

    let started = Instant::now();
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], port))),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_first_connect_timeout(Duration::from_secs(10)),
        )
        .await
        .expect("the attach connects within the timeout");
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the attach returned on the connect, not on the 10 s timer: {:?}",
        started.elapsed()
    );
    assert_eq!(transport.shared_instance(), Some(iface));
    assert!(transport.connected_to_shared_instance().await);

    assert!(transport.detach_shared_instance().await);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    timeout(WAIT, shared.stop()).await.expect("shared instance stop completes");
}
