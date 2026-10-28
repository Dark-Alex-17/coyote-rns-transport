//! The 0.17.0 README patch list promises, from outside the crate, that
//! `TcpClient::with_connect_events` and `LocalUnixClient::with_connect_events`
//! "with their `connect_events_enabled` getters fire on every established
//! stream, the first included" (`crates/libs/rns-transport/README.md`, the
//! 0.17.0 block of the patch list). Every other consumer-facing item of that
//! block has a `tests/*_api.rs` twin; the connect-event builders had none, so a
//! consumer following the register exercised only the shared-instance wrapper.
//! These pins drive the two clients the way the register names them: the
//! getter reports the installed sender, the FIRST stream fires a connect event
//! carrying the interface address while the sibling reconnect channel stays
//! silent (its rustdoc: "fires from the second on"), and the second stream
//! fires both.

use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

async fn recv_within(what: &str, rx: &mut mpsc::Receiver<AddressHash>) -> AddressHash {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

/// The sibling channel must stay silent across the first stream: a short
/// grace window after the connect event has already arrived is the only
/// witness an absence test can have.
async fn assert_silent(what: &str, rx: &mut mpsc::Receiver<AddressHash>) {
    assert!(
        timeout(Duration::from_millis(200), rx.recv()).await.is_err(),
        "{what} should not have fired"
    );
}

#[tokio::test]
async fn tcp_client_connect_events_fire_on_the_first_stream_and_every_later_one() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");

    assert!(
        !TcpClient::new(addr.to_string()).connect_events_enabled(),
        "a client built without the sender reports no connect events"
    );

    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);
    assert!(client.connect_events_enabled(), "the getter should report the installed sender");

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    // First stream: connect fires, reconnect does not.
    let first = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("first TCP connection timed out")
        .expect("first TCP connection failed")
        .0;
    let fired = recv_within("first connect event", &mut connect_rx).await;
    assert_eq!(fired, iface, "the connect event carries the interface address");
    assert_silent("the reconnect event on the first stream", &mut reconnect_rx).await;
    assert!(connect_rx.try_recv().is_err(), "exactly one connect per stream");

    // Second stream: both fire, same interface identity.
    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second TCP connection timed out")
        .expect("second TCP connection failed")
        .0;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);
    assert!(connect_rx.try_recv().is_err(), "exactly one connect per stream");

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

#[cfg(unix)]
#[tokio::test]
async fn local_unix_client_connect_events_fire_on_the_first_stream_and_every_later_one() {
    use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
    use tokio::net::UnixListener;

    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("connect-events.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");

    assert!(
        !LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
            .connect_events_enabled(),
        "a client built without the sender reports no connect events"
    );

    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);
    assert!(client.connect_events_enabled(), "the getter should report the installed sender");

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
    assert_silent("the reconnect event on the first stream", &mut reconnect_rx).await;
    assert!(connect_rx.try_recv().is_err(), "exactly one connect per stream");

    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second unix connection timed out")
        .expect("second unix connection failed")
        .0;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);
    assert!(connect_rx.try_recv().is_err(), "exactly one connect per stream");

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}
