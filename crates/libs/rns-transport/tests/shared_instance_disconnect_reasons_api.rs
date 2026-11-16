//! The opt-in disconnect reasons on the shared-instance state stream, used from outside
//! the crate against a raw `TcpListener` so the way each stream ends is the test's to
//! choose. With `SharedInstanceOptions::with_disconnect_reasons(true)` the stream reads
//! `DisconnectedWithReason { PeerClosed }` for an orderly close and
//! `DisconnectedWithReason { Reset }` for a linger-zero close, each where `Disconnected`
//! would sit and each followed by `Connecting`; a socket the peer merely holds quiet
//! sends nothing (the attach's clients run no read watchdog); the slot keeps naming the
//! interface across those drops and the detach still ends the stream without an event.
//! Without the opt-in, or with the opt-in switched back off, the same closes read the
//! plain `Disconnected`, so the README's consumer that matches on it keeps working; the
//! option follows the last `with_disconnect_reasons` call.

use std::time::Duration;

use rns_transport::transport::{
    DisconnectReason, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
use accept_within::accept_within;
#[path = "support/attach_with_events.rs"]
mod attach_with_events;
use attach_with_events::attach_with_events;
#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;
#[path = "support/close_orderly.rs"]
mod close_orderly;
use close_orderly::close_orderly;
#[path = "support/next_event.rs"]
mod next_event;
use next_event::next_event;
#[path = "support/serve_connected.rs"]
mod serve_connected;
use serve_connected::serve_connected;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// How long a held, quiet socket is watched for a spurious event. The attach's clients
/// run no read watchdog, so no length of silence should produce an event.
const QUIET: Duration = Duration::from_millis(400);

/// Closes `stream` with linger zero so the peer sends a reset instead of a FIN. The
/// client has no destination and no tunnel to synthesize, so nothing is pending on its
/// write side when the reset arrives; a write that failed first would report the reset
/// from the tx loop instead of the read.
fn close_with_reset(stream: TcpStream) {
    socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).expect("linger");
    drop(stream);
}

/// Reads the two events a served drop sends, asserts the second is `Connecting` and
/// returns the first, the one in the `Disconnected` position.
#[deny(clippy::await_holding_invalid_type)]
async fn read_drop(
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) -> SharedInstanceEvent {
    let dropped =
        next_event(what, events, WAIT).await.unwrap_or_else(|| panic!("{what}: the stream ended"));
    assert_eq!(
        next_event(what, events, WAIT).await,
        Some(SharedInstanceEvent::Connecting),
        "{what}: `Connecting` follows the drop at once"
    );
    dropped
}

/// Drives one attach through a held-quiet socket, an orderly close and a linger-zero
/// close and returns the two events read in the `Disconnected` position; the third
/// connection is read as `Connected` before the detach so no connect races it.
#[deny(clippy::await_holding_invalid_type)]
async fn hold_then_close_then_reset(
    name: &str,
    options: SharedInstanceOptions,
) -> [SharedInstanceEvent; 2] {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport(name);
    let (mut events, _) =
        attach_with_events(&transport, SharedInstanceEndpoint::Tcp(endpoint), options).await;
    let iface = transport.shared_instance().expect("the slot names the attached interface");

    let first = serve_connected(&listener, &mut events, "first connection").await;
    tokio::time::sleep(QUIET).await;
    assert_eq!(
        events.try_recv().err(),
        Some(TryRecvError::Empty),
        "a socket the peer holds quiet sends no event: the attach runs no read watchdog"
    );
    close_orderly(first).await;
    let after_close = read_drop(&mut events, "the orderly close").await;
    assert_eq!(
        transport.shared_instance(),
        Some(iface),
        "the slot keeps naming the interface across a drop"
    );

    // The reset has to land on a stream the client already holds; one that arrives
    // before its connect is served fails the dial instead.
    let second = serve_connected(&listener, &mut events, "second connection").await;
    close_with_reset(second);
    let after_reset = read_drop(&mut events, "the linger-zero close").await;
    assert_eq!(transport.shared_instance(), Some(iface));

    let third = serve_connected(&listener, &mut events, "third connection").await;
    assert!(transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(
        next_event("the stream's end", &mut events, WAIT).await,
        None,
        "the detach ends the stream without a disconnect event"
    );
    assert_eq!(transport.shared_instance(), None, "the detach clears the slot");
    drop(third);
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
    [after_close, after_reset]
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn opt_in_reads_peer_closed_for_a_close_and_reset_for_a_reset() {
    let read = hold_then_close_then_reset(
        "disconnect-reasons-opt-in",
        SharedInstanceOptions::default().with_disconnect_reasons(true),
    )
    .await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::DisconnectedWithReason { reason: DisconnectReason::PeerClosed },
            SharedInstanceEvent::DisconnectedWithReason { reason: DisconnectReason::Reset },
        ]
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn the_default_attach_keeps_reading_the_plain_disconnected() {
    let read =
        hold_then_close_then_reset("disconnect-reasons-default", SharedInstanceOptions::default())
            .await;
    assert_eq!(read, [SharedInstanceEvent::Disconnected, SharedInstanceEvent::Disconnected]);
}

/// The option follows the last call: an attach that opted in and then opted out again
/// reads the plain `Disconnected`, as the default does.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn opting_out_again_reads_the_plain_disconnected() {
    let read = hold_then_close_then_reset(
        "disconnect-reasons-opt-out-again",
        SharedInstanceOptions::default()
            .with_disconnect_reasons(true)
            .with_disconnect_reasons(false),
    )
    .await;
    assert_eq!(read, [SharedInstanceEvent::Disconnected, SharedInstanceEvent::Disconnected]);
}

/// The README's consumer shape: a `match` with a wildcard arm that labels each reason.
/// On an opt-in stream the `Disconnected` arm never fires and the reason arm reads the
/// label for the close the peer chose.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_readme_consumer_labels_each_reason_and_never_sees_the_plain_disconnected() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address");
    let transport = client_transport("disconnect-reasons-readme-consumer");
    let (labels_tx, mut labels) = mpsc::unbounded_channel::<&'static str>();
    let (events_tx, mut events) = mpsc::channel(16);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default()
                .with_reconnect_wait(RECONNECT_WAIT)
                .with_state_events(events_tx)
                .with_disconnect_reasons(true),
        )
        .await
        .expect("the attach is accepted");
    let follower = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            let label = match event {
                SharedInstanceEvent::Connecting => "connecting",
                SharedInstanceEvent::Connected => "connected",
                SharedInstanceEvent::Disconnected => "disconnected",
                SharedInstanceEvent::DisconnectedWithReason { reason } => match reason {
                    DisconnectReason::PeerClosed => "peer closed",
                    DisconnectReason::Reset => "reset",
                    DisconnectReason::Timeout => "timeout",
                    DisconnectReason::Io(_) => "io",
                    _ => "other reason",
                },
                _ => "other",
            };
            if labels_tx.send(label).is_err() {
                break;
            }
        }
    });

    #[deny(clippy::await_holding_invalid_type)]
    async fn next_label(labels: &mut mpsc::UnboundedReceiver<&'static str>) -> &'static str {
        timeout(WAIT, labels.recv())
            .await
            .expect("a label arrives")
            .expect("the follower is still reading")
    }
    // Each close lands on a stream the client already holds: a reset that arrives before
    // its connect is served is a failed dial, not a disconnect.
    assert_eq!(next_label(&mut labels).await, "connecting");
    let first = accept_within(&listener, "first connection").await;
    assert_eq!(next_label(&mut labels).await, "connected");
    close_orderly(first).await;
    assert_eq!(next_label(&mut labels).await, "peer closed");
    assert_eq!(next_label(&mut labels).await, "connecting");
    let second = accept_within(&listener, "second connection").await;
    assert_eq!(next_label(&mut labels).await, "connected");
    close_with_reset(second);
    assert_eq!(next_label(&mut labels).await, "reset");
    assert_eq!(next_label(&mut labels).await, "connecting");
    let third = accept_within(&listener, "third connection").await;
    assert_eq!(next_label(&mut labels).await, "connected");

    assert!(transport.detach_shared_instance().await);
    timeout(WAIT, follower).await.expect("the stream ends on the detach").expect("follower ends");
    assert_eq!(labels.recv().await, None, "nothing after the detach");
    drop(third);
    drop(listener);
    timeout(WAIT, transport.stop()).await.expect("client stop completes");
}
