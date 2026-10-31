// Coverage for the opt-in disconnect reasons on the shared-instance state stream, read
// over a raw listener so the way each stream ends is the test's to choose: with
// `with_disconnect_reasons(true)` an orderly close reads `DisconnectedWithReason`
// carrying `PeerClosed` and a linger-zero close one carrying `Reset`, each where the
// plain `Disconnected` would sit; without the opt-in the same closes read the plain
// `Disconnected`; the unix client reports `PeerClosed` the same way on an orderly close.
// The consumer driven by hand pins the reason's position within one cycle and on a drop
// that was queued before it ran. Helpers come from the sibling `tests_parts` files
// included into this module.

use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};
use tokio::net::TcpListener as DisconnectReasonsTcpListener;

/// The reconnect wait of the clients below: short, so the redial after each close lands
/// well inside `LOOPBACK_TIMEOUT`.
const SHARED_INSTANCE_DISCONNECT_REASONS_RECONNECT_WAIT: Duration = Duration::from_millis(50);

/// Attaches `name` to `endpoint` with a state stream of capacity 16, the short reconnect
/// wait and `disconnect_reasons` as given; returns the client and the stream's receiver.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_client(
    name: &str,
    endpoint: SharedInstanceEndpoint,
    disconnect_reasons: bool,
) -> (SharedInstanceClient, tokio::sync::mpsc::Receiver<SharedInstanceEvent>) {
    let (events_tx, events) = tokio::sync::mpsc::channel(16);
    let client = shared_instance_client_with_options(
        name,
        endpoint,
        SharedInstanceOptions::default()
            .with_reconnect_wait(SHARED_INSTANCE_DISCONNECT_REASONS_RECONNECT_WAIT)
            .with_state_events(events_tx)
            .with_disconnect_reasons(disconnect_reasons),
    )
    .await;
    (client, events)
}

/// Accepts the client's next connection on `listener` within `LOOPBACK_TIMEOUT`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_accept(
    listener: &DisconnectReasonsTcpListener,
) -> tokio::net::TcpStream {
    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the client to connect")
        .expect("accept");
    stream
}

/// Closes `stream` in an orderly way: the write side is shut down and the stream read to
/// the client's EOF before it is dropped. The client writes its tunnel packet on each
/// connect, shortly after `Connected` is served, and a socket dropped with unread data in
/// its receive buffer reports a reset to its peer rather than a plain close (TCP sends
/// RST; Linux sets `ECONNRESET` on a unix socket's peer), which the client would read as
/// `Reset`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_close_orderly<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
) {
    stream.shutdown().await.expect("shut the write side down");
    timeout(LOOPBACK_TIMEOUT, async {
        let mut sink = [0u8; 1024];
        while stream.read(&mut sink).await.expect("drain the client's writes") > 0 {}
    })
    .await
    .expect("timed out waiting for the client to close its side");
}

/// Reads the tunnel packet the client writes on connect from `stream`, within
/// `LOOPBACK_TIMEOUT`.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_read_tunnel_packet(stream: &mut tokio::net::TcpStream) {
    let mut buf = [0u8; 1024];
    let read = timeout(LOOPBACK_TIMEOUT, stream.read(&mut buf))
        .await
        .expect("timed out waiting for the client's tunnel packet")
        .expect("read the client's tunnel packet");
    assert!(read > 0, "the client wrote its tunnel packet before the stream ended");
}

/// Reads the two events a served drop sends, asserts the second is `Connecting` and
/// returns the first, the one in the `Disconnected` position.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_read_drop(
    events: &mut tokio::sync::mpsc::Receiver<SharedInstanceEvent>,
) -> SharedInstanceEvent {
    let dropped = shared_instance_state_next_cycle_event(events).await.expect("the drop is served");
    assert_eq!(
        shared_instance_state_next_cycle_event(events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    dropped
}

/// Drives one TCP client through an orderly close and a linger-zero close over a raw
/// listener and returns the two events read in the `Disconnected` position; the first
/// stream is closed orderly, the second with linger zero so the peer sends a reset. The
/// third connection is accepted and read as `Connected` before the detach, so no connect
/// is left racing the detach, and the detach ends the stream with nothing further.
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_tcp_close_then_reset(
    name: &str,
    disconnect_reasons: bool,
) -> [SharedInstanceEvent; 2] {
    let listener = DisconnectReasonsTcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let (client, mut events) = shared_instance_disconnect_reasons_client(
        name,
        SharedInstanceEndpoint::Tcp(addr),
        disconnect_reasons,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    let first = shared_instance_disconnect_reasons_accept(&listener).await;
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));
    shared_instance_disconnect_reasons_close_orderly(first).await;
    let after_close = shared_instance_disconnect_reasons_read_drop(&mut events).await;

    // The reset has to land on a stream the client already holds; one that arrives before
    // its connect is served fails the dial instead. Its tunnel packet is read first so
    // nothing is pending on the client's write side when the reset arrives: a write that
    // failed first would report the reset from the tx loop instead of the read.
    let mut second = shared_instance_disconnect_reasons_accept(&listener).await;
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );
    shared_instance_disconnect_reasons_read_tunnel_packet(&mut second).await;
    socket2::SockRef::from(&second).set_linger(Some(Duration::ZERO)).expect("linger");
    drop(second);
    let after_reset = shared_instance_disconnect_reasons_read_drop(&mut events).await;

    let third = shared_instance_disconnect_reasons_accept(&listener).await;
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );
    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        None,
        "the detach ends the stream without a disconnect event"
    );
    drop(third);
    drop(listener);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
    [after_close, after_reset]
}

/// With the opt-in, a peer's orderly close reads `DisconnectedWithReason { PeerClosed }`
/// and its linger-zero close `DisconnectedWithReason { Reset }`, each followed by
/// `Connecting`, where the plain stream reads `Disconnected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_tcp_peer_close_reads_peer_closed_then_a_reset_reads_reset(
) {
    let read = shared_instance_disconnect_reasons_tcp_close_then_reset(
        "shared-instance-disconnect-reasons-tcp",
        true,
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

/// Without the opt-in the same two closes read the plain `Disconnected`: the reason the
/// client reported is dropped and the variant is the one a caller already matches on.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_opt_out_still_reads_disconnected() {
    let read = shared_instance_disconnect_reasons_tcp_close_then_reset(
        "shared-instance-disconnect-reasons-opt-out",
        false,
    )
    .await;
    assert_eq!(read, [SharedInstanceEvent::Disconnected, SharedInstanceEvent::Disconnected]);
}

/// The whole cycle on a hand-driven consumer that sends the reason: `DisconnectedWithReason`
/// takes the place of `Disconnected`, with the `PeerClosed` the driver queues, and the rest
/// of the stream is what `shared_instance_state_consumer_orders_a_whole_cycle` reads.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_consumer_orders_a_whole_cycle_with_the_reason() {
    let read = shared_instance_state_drive_consumer_with(
        "shared-instance-disconnect-reasons-consumer-cycle",
        DriveSpec {
            queued: &[Queued::Disconnect, Queued::Connect, Queued::Reconnect],
            close_before_run: &[Queued::Reconnect],
            cancel_before_run: false,
            first_connect_abandoned: false,
            disconnect_reasons: true,
            exits: "the consumer exits once the reconnect channel is drained and closed",
        },
    )
    .await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::DisconnectedWithReason { reason: DisconnectReason::PeerClosed },
            SharedInstanceEvent::Connecting,
            SharedInstanceEvent::Connected
        ]
    );
}

/// A drop queued before the consumer runs, with every sender gone, is served as
/// `DisconnectedWithReason { PeerClosed }` then `Connecting` under the opt-in: the biased
/// select's live disconnect arm yields the buffered drop before the closed channel reads
/// `None`, so the reason rides it the same way it does one served while the client runs.
/// The twin of `shared_instance_state_consumer_serves_a_queued_drop_before_the_stream_ends`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_consumer_serves_a_queued_drop_with_the_reason() {
    let read = shared_instance_state_drive_consumer_with(
        "shared-instance-disconnect-reasons-consumer-queued-drop",
        DriveSpec {
            queued: &[Queued::Disconnect],
            close_before_run: &[Queued::Reconnect, Queued::Connect, Queued::Disconnect],
            cancel_before_run: false,
            first_connect_abandoned: false,
            disconnect_reasons: true,
            exits: "the consumer exits once the queued drop is served and the channels are closed",
        },
    )
    .await;
    assert_eq!(
        read,
        [
            SharedInstanceEvent::DisconnectedWithReason { reason: DisconnectReason::PeerClosed },
            SharedInstanceEvent::Connecting
        ],
        "the queued drop is served with its reason before the stream ends"
    );
}

/// The unix twin: a peer's orderly close on a raw `UnixListener` reads
/// `DisconnectedWithReason { PeerClosed }` then `Connecting`; the redial is accepted and
/// read as `Connected` before the detach ends the stream.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_disconnect_reasons_unix_peer_close_reads_peer_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket = temp.path().join("reasons.sock");
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the socket");
    let (client, mut events) = shared_instance_disconnect_reasons_client(
        "shared-instance-disconnect-reasons-unix",
        SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.clone())),
        true,
    )
    .await;
    assert_eq!(
        shared_instance_state_next(&mut events).await,
        Some(SharedInstanceEvent::Connecting)
    );
    let (stream, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to connect")
        .expect("accept");
    assert_eq!(shared_instance_state_next(&mut events).await, Some(SharedInstanceEvent::Connected));

    shared_instance_disconnect_reasons_close_orderly(stream).await;
    assert_eq!(
        shared_instance_disconnect_reasons_read_drop(&mut events).await,
        SharedInstanceEvent::DisconnectedWithReason { reason: DisconnectReason::PeerClosed }
    );

    let (again, _) = timeout(LOOPBACK_TIMEOUT, listener.accept())
        .await
        .expect("timed out waiting for the unix client to redial")
        .expect("accept");
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        Some(SharedInstanceEvent::Connected)
    );
    assert!(client.transport.detach_shared_instance().await, "the attach is in place");
    assert_eq!(
        shared_instance_state_next_cycle_event(&mut events).await,
        None,
        "the detach ends the stream without a disconnect event"
    );
    drop(again);
    timeout(STOP_TIMEOUT, client.transport.stop()).await.expect("client stop completes");
}
