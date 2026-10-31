//! The disconnect-reason surface used from outside the crate:
//! `TcpClient::with_disconnect_reason_events` and
//! `LocalUnixClient::with_disconnect_reason_events` report each established
//! stream that ends while the client is neither cancelled nor stopped as
//! `(interface address, DisconnectReason)`, beside the plain disconnect
//! channel, which keeps firing. A peer that drops its end reads
//! `PeerClosed`; a peer that closes with linger zero sends a reset and reads
//! `Reset`. A client cancelled while connected sends nothing on the reason
//! channel. A full channel drops the reason and nothing else: the client
//! keeps reconnecting. Both clients answer
//! `disconnect_reason_events_enabled()` false until the builder sets the
//! channel.

use std::time::Duration;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::transport::DisconnectReason;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::timeout;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);

#[deny(clippy::await_holding_invalid_type)]
async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

/// Once the client task has ended its sender is gone, so the channel must
/// close with nothing queued on it.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_closed_without_event<T: std::fmt::Debug + PartialEq>(
    what: &str,
    rx: &mut mpsc::Receiver<T>,
) {
    assert_eq!(
        timeout(Duration::from_secs(2), rx.recv()).await.expect("the reason channel stayed open"),
        None,
        "{what} should not have fired"
    );
}

#[deny(clippy::await_holding_invalid_type)]
async fn accept_within(listener: &TcpListener, what: &str) -> TcpStream {
    timeout(Duration::from_secs(2), listener.accept())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} failed: {err}"))
        .0
}

#[deny(clippy::await_holding_invalid_type)]
async fn join_within(task: tokio::task::JoinHandle<()>, what: &str) {
    timeout(Duration::from_secs(2), task)
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} panicked: {err}"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_reports_peer_closed_then_reset_with_the_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let (plain_tx, mut plain_rx) = mpsc::channel::<AddressHash>(4);
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_disconnect_events(plain_tx)
        .with_connect_events(connect_tx);
    assert!(!client.disconnect_reason_events_enabled(), "no reason channel by default");
    let client = client.with_disconnect_reason_events(reason_tx);
    assert!(client.disconnect_reason_events_enabled(), "the builder sets the channel");

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let first = accept_within(&listener, "first TCP connection").await;
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );
    assert_eq!(
        recv_within("plain disconnect event", &mut plain_rx).await,
        iface,
        "the plain channel fires beside the reason channel"
    );

    // The reset has to land on a stream the client already holds; one that
    // arrives before its connect has completed fails the dial instead.
    let second = accept_within(&listener, "second TCP connection").await;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    socket2::SockRef::from(&second).set_linger(Some(Duration::ZERO)).expect("linger");
    drop(second);
    assert_eq!(recv_within("reset reason", &mut reason_rx).await, (iface, DisconnectReason::Reset));
    assert_eq!(recv_within("plain disconnect event", &mut plain_rx).await, iface);

    let third = accept_within(&listener, "third TCP connection").await;
    assert_eq!(recv_within("third connect event", &mut connect_rx).await, iface);
    cancel.cancel();
    drop(listener);
    join_within(task, "TCP client task").await;
    drop(third);
    assert_closed_without_event("a reason for a cancelled client", &mut reason_rx).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_stopped_while_connected_sends_no_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let stream = accept_within(&listener, "TCP connection").await;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    cancel.cancel();
    join_within(task, "TCP client task").await;
    assert_closed_without_event("a reason for a cancelled client", &mut reason_rx).await;
    drop(stream);
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_reports_peer_closed_with_the_reason() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("disconnect-reason.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx);
    assert!(!client.disconnect_reason_events_enabled(), "no reason channel by default");
    let client = client.with_disconnect_reason_events(reason_tx);
    assert!(client.disconnect_reason_events_enabled(), "the builder sets the channel");

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    let first = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("first unix connection timed out")
        .expect("first unix connection failed")
        .0;
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );

    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second unix connection timed out")
        .expect("second unix connection failed")
        .0;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    cancel.cancel();
    join_within(task, "unix client task").await;
    drop(second);
    assert_closed_without_event("a reason for a cancelled client", &mut reason_rx).await;
}

/// A full reason channel (capacity one, never read) drops the second
/// reason and nothing else: the client reconnects a third time.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_drops_the_reason_on_a_full_channel_and_keeps_reconnecting() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let (reason_tx, mut reason_rx) = mpsc::channel(1);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let first = accept_within(&listener, "first TCP connection").await;
    drop(first);
    let second = accept_within(&listener, "second TCP connection").await;
    drop(second);
    let third = accept_within(
        &listener,
        "third TCP connection: a full reason channel must not stop the client",
    )
    .await;

    assert_eq!(
        reason_rx.try_recv().ok(),
        Some((iface, DisconnectReason::PeerClosed)),
        "the first reason is buffered"
    );
    assert!(reason_rx.try_recv().is_err(), "the second reason was dropped, not queued");

    cancel.cancel();
    drop(third);
    join_within(task, "TCP client task").await;
}
