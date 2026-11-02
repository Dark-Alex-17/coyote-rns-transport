//! The dial-failure channel the README names under "A client attached while
//! nothing listens also reports each failed attempt", driven from outside the
//! crate the way `tests/dial_failure_events_api.rs` drives it, for the
//! patterns that file leaves unpinned:
//!
//! - the CHANGELOG says "a full or closed channel drops the event at debug,
//!   as the connect events do": a consumer who drops the receiver, or who
//!   never reads a capacity-one channel, must still see the client keep
//!   dialing and connect once the socket appears, with exactly one buffered
//!   failure in the capacity-one case (the connect-event twins live in
//!   `tests/connect_events_api.rs`);
//! - a stale socket file gives `Refused`: a path whose listener is gone but
//!   whose file remains reads `Refused`, not `NotFound`, so a consumer can
//!   tell "nothing ever bound here" from "something bound here and went
//!   away";
//! - the README says the three reason types "live in `iface::events` and
//!   are re-exported from `transport`": both paths name the same types, and
//!   `DisconnectReason`, which no client emits yet, is `Copy + Send + Sync`
//!   like its siblings.

use std::time::Duration;

#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);

/// A capacity-one dial-failure channel that nobody reads holds the first
/// failure and drops every later one; the client is not slowed or stopped by
/// the full channel and connects as soon as the port is bound.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_keeps_dialing_when_the_dial_failure_channel_is_full() {
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, mut failure_rx) = mpsc::channel(1);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    // Several attempts fail against the full channel before the port exists.
    tokio::time::sleep(RECONNECT_WAIT * 6).await;
    assert!(connect_rx.try_recv().is_err(), "nothing connected while the port was closed");

    let listener = TcpListener::bind(&addr).await.expect("bind the port the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out: a full dial-failure channel must not stop the client")
        .expect("TCP connection failed")
        .0;
    assert_eq!(
        timeout(Duration::from_secs(2), connect_rx.recv())
            .await
            .expect("connect event timed out")
            .expect("connect channel closed"),
        iface,
        "the connect event is served although the dial-failure channel is full"
    );

    let buffered = failure_rx.try_recv().expect("the first failure is buffered");
    assert_eq!(buffered.iface(), iface);
    assert_eq!(buffered.kind(), rns_transport::transport::DialFailureKind::Refused);
    assert!(failure_rx.try_recv().is_err(), "every later failure was dropped, not queued");

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// A dial-failure receiver dropped before the first attempt must not stop
/// the client: it keeps dialing and connects once the port is bound.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_keeps_dialing_when_the_dial_failure_receiver_is_gone() {
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, failure_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    drop(failure_rx);
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    tokio::time::sleep(RECONNECT_WAIT * 4).await;
    let listener = TcpListener::bind(&addr).await.expect("bind the port the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out: a closed dial-failure channel must not stop the client")
        .expect("TCP connection failed")
        .0;
    assert_eq!(
        timeout(Duration::from_secs(2), connect_rx.recv())
            .await
            .expect("connect event timed out")
            .expect("connect channel closed"),
        iface
    );

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// The unix twin of the full-channel pin: a never-read capacity-one channel
/// buffers the first `NotFound` and drops the rest; the client still connects
/// once the path is bound.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_keeps_dialing_when_the_dial_failure_channel_is_full() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("full-channel.sock");
    let (failure_tx, mut failure_rx) = mpsc::channel(1);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    tokio::time::sleep(RECONNECT_WAIT * 6).await;
    let listener = UnixListener::bind(&socket).expect("bind the socket the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out: a full dial-failure channel must not stop the client")
        .expect("unix connection failed")
        .0;
    assert_eq!(
        timeout(Duration::from_secs(2), connect_rx.recv())
            .await
            .expect("connect event timed out")
            .expect("connect channel closed"),
        iface
    );
    let buffered = failure_rx.try_recv().expect("the first failure is buffered");
    assert_eq!(buffered.kind(), rns_transport::transport::DialFailureKind::NotFound);
    assert!(failure_rx.try_recv().is_err(), "every later failure was dropped, not queued");

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

/// A socket file left behind by a listener that is gone reads `Refused`,
/// not `NotFound`: the path exists, nothing accepts there.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_reads_refused_for_a_stale_socket_file() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("stale.sock");
    // Bind once and drop the listener; tokio's UnixListener does not unlink
    // the file on drop, so the stale path remains.
    drop(UnixListener::bind(&socket).expect("bind then abandon the socket"));
    assert!(socket.exists(), "the stale socket file is the fixture");

    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    for attempt in 1..=2 {
        let failure = timeout(Duration::from_secs(2), failure_rx.recv())
            .await
            .unwrap_or_else(|_| panic!("dial failure {attempt} timed out"))
            .unwrap_or_else(|| panic!("dial failure channel closed on attempt {attempt}"));
        assert_eq!(failure.iface(), iface);
        assert_eq!(
            failure.kind(),
            rns_transport::transport::DialFailureKind::Refused,
            "attempt {attempt}: a stale socket file is refused, not missing"
        );
    }

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

/// `iface::events` and the `transport` re-export name the same three types,
/// and the reserved `DisconnectReason` is `Copy + Send + Sync` like its
/// siblings.
#[test]
fn the_reason_types_are_one_set_under_both_paths_and_all_copy() {
    fn same<T>(_: T, _: T) {}
    fn is_copy<T: Copy>() {}
    fn is_send_sync<T: Send + Sync>() {}

    same(
        rns_transport::iface::events::DialFailureKind::Refused,
        rns_transport::transport::DialFailureKind::Refused,
    );
    same(
        rns_transport::iface::events::DisconnectReason::PeerClosed,
        rns_transport::transport::DisconnectReason::PeerClosed,
    );
    let (tx, _rx) = mpsc::channel::<rns_transport::iface::events::DialFailure>(1);
    let _: mpsc::Sender<rns_transport::transport::DialFailure> = tx;

    is_copy::<rns_transport::transport::DisconnectReason>();
    is_send_sync::<rns_transport::transport::DisconnectReason>();
    is_send_sync::<rns_transport::transport::DialFailureKind>();

    // `Io` is the catch-all; the named kinds compare apart from it.
    assert_ne!(
        rns_transport::transport::DialFailureKind::Io(std::io::ErrorKind::Other),
        rns_transport::transport::DialFailureKind::Refused
    );
    assert_ne!(
        rns_transport::transport::DisconnectReason::Io(std::io::ErrorKind::Other),
        rns_transport::transport::DisconnectReason::Reset
    );
}
