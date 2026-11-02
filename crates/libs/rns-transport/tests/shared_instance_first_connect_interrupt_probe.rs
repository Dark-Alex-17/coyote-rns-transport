//! The `with_first_connect_timeout` rustdoc's second arm, followed from outside
//! the crate: the attach "detaches again, as it does when the attach's
//! consumer ends first (a stop, detach or `stop_interface` during the wait),
//! and returns `RnsError::SharedInstanceConnectTimeout`".
//! `tests/shared_instance_first_connect_api.rs` pins the detach-and-replace
//! shape on a 1 s timer, where the consumer-gone wake and the timer are
//! indistinguishable; these pins give the timer 10 s and hold that an
//! interrupted wait answers well ahead of it, that a `stop` during the wait
//! is one such interruption, and that both leave the slot empty, the posture
//! off, and the stream ended without a `Connected`.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::error::RnsError;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions,
};
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;

const WAIT: Duration = Duration::from_secs(15);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// The first-connect timer the interrupted attaches are given; the pins hold
/// that they answer in a fraction of it.
const LONG_TIMER: Duration = Duration::from_secs(10);
/// The most an interrupted attach may take to answer: a few reconnect waits
/// plus the stop budget's worst case is still far inside `LONG_TIMER`.
const PROMPT: Duration = Duration::from_secs(5);

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.expect("a state event arrives in time")
}

/// Drains the stream to its end and holds that nothing but `DialFailed`
/// follows the `Connecting` already read.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_only_failures_then_end(events: &mut mpsc::Receiver<SharedInstanceEvent>) {
    let mut rest = Vec::new();
    while let Some(event) = next_event(events).await {
        rest.push(event);
    }
    assert!(
        rest.iter().all(|event| matches!(event, SharedInstanceEvent::DialFailed { .. })),
        "only dial failures follow Connecting before the stream ends: {rest:?}"
    );
}

/// A detach taken during the wait, with nothing attached after it: the attach
/// answers `SharedInstanceConnectTimeout` ahead of its 10 s timer, the slot is
/// empty, the posture off, a second detach finds nothing, and the stream ends
/// on refusals alone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn detach_during_the_first_connect_wait_answers_ahead_of_the_timer() {
    let dead_port = free_loopback_port().await;
    let transport = client_transport("first-connect-detached-client");
    let (events_tx, mut events) = mpsc::channel(16);

    let started = Instant::now();
    let attach = transport.attach_shared_instance(
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], dead_port))),
        SharedInstanceOptions::default()
            .with_reconnect_wait(RECONNECT_WAIT)
            .with_state_events(events_tx)
            .with_first_connect_timeout(LONG_TIMER),
    );
    let interrupt = async {
        assert_eq!(
            next_event(&mut events).await,
            Some(SharedInstanceEvent::Connecting),
            "the attach holds the slot once its Connecting is out"
        );
        assert!(transport.detach_shared_instance().await, "the detach takes the waiting attach");
    };
    let (attach, ()) = tokio::join!(attach, interrupt);
    let elapsed = started.elapsed();
    assert!(
        matches!(attach, Err(RnsError::SharedInstanceConnectTimeout)),
        "a detach during the wait reads as the connect timeout: {attach:?}"
    );
    assert!(
        elapsed < PROMPT,
        "the attach answered on the detach, not on the {LONG_TIMER:?} timer: {elapsed:?}"
    );
    assert_eq!(transport.shared_instance(), None, "the slot is empty");
    assert!(!transport.connected_to_shared_instance().await, "the posture is off");
    assert!(!transport.detach_shared_instance().await, "a second detach finds nothing");
    assert_only_failures_then_end(&mut events).await;

    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}

/// A `stop` taken during the wait: the attach answers
/// `SharedInstanceConnectTimeout` ahead of its 10 s timer rather than
/// outliving the stopped transport, the slot is empty, the posture off, and
/// the stream ends on refusals alone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn stop_during_the_first_connect_wait_answers_ahead_of_the_timer() {
    let dead_port = free_loopback_port().await;
    let transport = client_transport("first-connect-stopped-client");
    let (events_tx, mut events) = mpsc::channel(16);

    let started = Instant::now();
    let attach = transport.attach_shared_instance(
        SharedInstanceEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], dead_port))),
        SharedInstanceOptions::default()
            .with_reconnect_wait(RECONNECT_WAIT)
            .with_state_events(events_tx)
            .with_first_connect_timeout(LONG_TIMER),
    );
    let interrupt = async {
        assert_eq!(
            next_event(&mut events).await,
            Some(SharedInstanceEvent::Connecting),
            "the attach holds the slot once its Connecting is out"
        );
        timeout(WAIT, transport.stop()).await.expect("stop completes during the wait");
    };
    let (attach, ()) = tokio::join!(attach, interrupt);
    let elapsed = started.elapsed();
    assert!(
        matches!(attach, Err(RnsError::SharedInstanceConnectTimeout)),
        "a stop during the wait reads as the connect timeout: {attach:?}"
    );
    assert!(
        elapsed < PROMPT,
        "the attach answered on the stop, not on the {LONG_TIMER:?} timer: {elapsed:?}"
    );
    assert_eq!(transport.shared_instance(), None, "the slot is empty after the stop");
    assert!(!transport.connected_to_shared_instance().await, "the posture is off");
    assert!(!transport.detach_shared_instance().await, "a detach after the stop finds nothing");
    assert_only_failures_then_end(&mut events).await;
}
