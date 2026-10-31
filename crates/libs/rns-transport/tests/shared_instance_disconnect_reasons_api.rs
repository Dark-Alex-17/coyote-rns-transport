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

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{
    DisconnectReason, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions,
    Transport, TransportConfig,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::timeout;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// How long a held, quiet socket is watched for a spurious event. The attach's clients
/// run no read watchdog, so no length of silence should produce an event.
const QUIET: Duration = Duration::from_millis(400);

fn client_transport(name: &str) -> Arc<Transport> {
    Arc::new(Transport::new(TransportConfig::new(
        name,
        &PrivateIdentity::new_from_rand(OsRng),
        false,
    )))
}

#[deny(clippy::await_holding_invalid_type)]
async fn next_event(
    what: &str,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
) -> Option<SharedInstanceEvent> {
    timeout(WAIT, events.recv()).await.unwrap_or_else(|_| panic!("{what} timed out"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(WAIT, listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}

/// Closes `stream` in an orderly way: the write side is shut down and the stream read to
/// the client's EOF before it is dropped. The client has written its tunnel packet by
/// the time it is connected, and a TCP socket dropped with unread data in its receive
/// buffer sends a reset rather than a FIN.
#[deny(clippy::await_holding_invalid_type)]
async fn close_orderly(mut stream: TcpStream) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(WAIT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the client's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the client to close its side");
}

/// Closes `stream` with linger zero so the peer sends a reset instead of a FIN.
fn close_with_reset(stream: TcpStream) {
    socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).expect("linger");
    drop(stream);
}

/// Attaches `transport` to the raw listener at `endpoint` with `options` plus a
/// 16-slot state stream and the short reconnect wait, and reads the attach's
/// `Connecting`; returns the stream's receiver.
#[deny(clippy::await_holding_invalid_type)]
async fn attach(
    transport: &Transport,
    endpoint: SocketAddr,
    options: SharedInstanceOptions,
) -> mpsc::Receiver<SharedInstanceEvent> {
    let (events_tx, mut events) = mpsc::channel(16);
    transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            options.with_reconnect_wait(RECONNECT_WAIT).with_state_events(events_tx),
        )
        .await
        .expect("the attach is accepted");
    assert_eq!(
        next_event("the attach's Connecting", &mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    events
}

/// Serves one connection on `listener` and reads `Connected` for it.
#[deny(clippy::await_holding_invalid_type)]
async fn serve_connected(
    listener: &TcpListener,
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) -> TcpStream {
    let stream = accept_within(listener, what).await;
    assert_eq!(next_event(what, events).await, Some(SharedInstanceEvent::Connected), "{what}");
    stream
}

/// Reads the two events a served drop sends, asserts the second is `Connecting` and
/// returns the first, the one in the `Disconnected` position.
#[deny(clippy::await_holding_invalid_type)]
async fn read_drop(
    events: &mut mpsc::Receiver<SharedInstanceEvent>,
    what: &str,
) -> SharedInstanceEvent {
    let dropped =
        next_event(what, events).await.unwrap_or_else(|| panic!("{what}: the stream ended"));
    assert_eq!(
        next_event(what, events).await,
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
    let mut events = attach(&transport, endpoint, options).await;
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
        next_event("the stream's end", &mut events).await,
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
