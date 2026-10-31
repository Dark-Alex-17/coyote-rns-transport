//! The third interruption the `with_first_connect_timeout` rustdoc names,
//! followed from outside the crate: the attach "detaches again, as it does
//! when the attach's consumer ends first (a stop, detach or `stop_interface`
//! during the wait), and returns `RnsError::SharedInstanceConnectTimeout`".
//! `tests/shared_instance_first_connect_interrupt_probe.rs` pins the detach
//! and the `stop`; this pin holds the same shape for a `stop_interface` taken
//! on the attached interface during the wait: the attach answers well ahead
//! of its 10 s timer, the slot is empty afterwards (the timeout cleanup
//! releases what a bare `stop_interface` would have left held), the posture
//! is off, and the stream ends on refusals alone.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rns_transport::error::RnsError;
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions, Transport, TransportConfig,
};
use tokio::sync::mpsc;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(15);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// The first-connect timer the interrupted attach is given; the pin holds
/// that it answers in a fraction of it.
const LONG_TIMER: Duration = Duration::from_secs(10);
/// The most the interrupted attach may take to answer, far inside `LONG_TIMER`.
const PROMPT: Duration = Duration::from_secs(5);

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

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.expect("a state event arrives in time")
}

/// A `stop_interface` on the attached interface during the wait: the attach
/// answers `SharedInstanceConnectTimeout` ahead of its 10 s timer, the slot
/// it would otherwise have kept held is empty, the posture is off, a detach
/// afterwards finds nothing, and the stream ends on refusals alone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn stop_interface_during_the_first_connect_wait_answers_ahead_of_the_timer() {
    let dead_port = free_loopback_port().await;
    let transport = client_transport("first-connect-stop-interface-client");
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
        let iface = transport.shared_instance().expect("the waiting attach holds the slot");
        assert!(
            transport.stop_interface(iface).await,
            "stop_interface removes the interface the attach is waiting on"
        );
    };
    let (attach, ()) = tokio::join!(attach, interrupt);
    let elapsed = started.elapsed();
    assert!(
        matches!(attach, Err(RnsError::SharedInstanceConnectTimeout)),
        "a stop_interface during the wait reads as the connect timeout: {attach:?}"
    );
    assert!(
        elapsed < PROMPT,
        "the attach answered on the stop_interface, not on the {LONG_TIMER:?} timer: {elapsed:?}"
    );
    assert_eq!(
        transport.shared_instance(),
        None,
        "the timeout cleanup released the slot a bare stop_interface leaves held"
    );
    assert!(!transport.connected_to_shared_instance().await, "the posture is off");
    assert!(!transport.detach_shared_instance().await, "a detach afterwards finds nothing");
    let mut rest = Vec::new();
    while let Some(event) = next_event(&mut events).await {
        rest.push(event);
    }
    assert!(
        rest.iter().all(|event| matches!(event, SharedInstanceEvent::DialFailed { .. })),
        "only dial failures follow Connecting before the stream ends: {rest:?}"
    );

    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}
