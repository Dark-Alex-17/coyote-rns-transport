//! The README's dial-failure surface used from outside the crate:
//! `TcpClient::with_dial_failure_events` and
//! `LocalUnixClient::with_dial_failure_events` report every failed connect
//! attempt as a `DialFailure` naming the interface and a `DialFailureKind`,
//! `Refused` for a TCP port nothing listens on and `NotFound` for a unix
//! socket path nothing has bound, one per attempt until the socket appears,
//! after which the connect event fires and the dial-failure channel goes
//! quiet. The TCP pin also reads, through a capturing `log::Log` installed
//! once for the binary, that each failed attempt warns with the endpoint
//! and the io error's own text, "Connection refused" for a closed port, so
//! a reader of the log can tell a refusal from a timeout; the records are
//! attributed by the endpoint they name. The last pin holds at compile time that `DialFailure` is
//! `Copy + Send + Sync` and that `SharedInstanceEvent` stayed `Copy` with its
//! new `DialFailed` variant.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::transport::{DialFailure, DialFailureKind, SharedInstanceEvent};
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);

struct CaptureLogger {
    records: Mutex<Vec<(log::Level, String)>>,
}

static CAPTURED: CaptureLogger = CaptureLogger { records: Mutex::new(Vec::new()) };

impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with("rns_transport::iface")
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.records
            .lock()
            .expect("captured record store")
            .push((record.level(), record.args().to_string()));
    }

    fn flush(&self) {}
}

fn install_capture_logger() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&CAPTURED).expect("no other logger in this test binary");
        log::set_max_level(log::LevelFilter::Trace);
    });
}

/// Every captured warn whose message starts with the failed-dial line for `endpoint`.
fn failed_dial_warns(endpoint: &str) -> Vec<String> {
    let needle = format!("couldn't connect to <{endpoint}>: ");
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(level, message)| *level == log::Level::Warn && message.contains(&needle))
        .map(|(_, message)| message.clone())
        .collect()
}

async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

/// Once the connect has fired, the dial-failure channel must stay quiet; a
/// grace window of several reconnect waits is the only witness an absence
/// test can have.
async fn assert_silent<T>(what: &str, rx: &mut mpsc::Receiver<T>) {
    assert!(
        timeout(Duration::from_millis(300), rx.recv()).await.is_err(),
        "{what} should not have fired"
    );
}

async fn free_loopback_port() -> u16 {
    let probe = TcpListener::bind("127.0.0.1:0").await.expect("bind probe");
    let port = probe.local_addr().expect("probe address").port();
    drop(probe);
    port
}

/// Two failures in a row with the expected kind on `iface`: the client
/// keeps dialing, one event per attempt.
async fn assert_two_failures(
    rx: &mut mpsc::Receiver<DialFailure>,
    iface: AddressHash,
    kind: DialFailureKind,
) {
    for attempt in 1..=2 {
        let failure = recv_within(&format!("dial failure {attempt}"), rx).await;
        assert_eq!(failure.iface(), iface, "the failure names the interface");
        assert_eq!(failure.kind(), kind, "attempt {attempt}");
    }
}

#[tokio::test]
async fn tcp_client_reports_refused_dials_until_a_listener_binds_the_port() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
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

    assert_two_failures(&mut failure_rx, iface, DialFailureKind::Refused).await;
    assert!(connect_rx.try_recv().is_err(), "nothing connected while the port was closed");
    let warns = failed_dial_warns(&addr);
    assert!(warns.len() >= 2, "one warn per failed attempt naming the endpoint: {warns:?}");
    for message in &warns {
        assert!(
            message.contains("Connection refused"),
            "the warn carries the io error's text: {message}"
        );
    }

    let listener = TcpListener::bind(&addr).await.expect("bind the port the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out")
        .expect("TCP connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    // Failures queued before the bind landed are allowed; none may follow the connect.
    while failure_rx.try_recv().is_ok() {}
    assert_silent("a dial failure after the connect", &mut failure_rx).await;

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

#[cfg(unix)]
#[tokio::test]
async fn local_unix_client_reports_not_found_dials_until_the_socket_is_bound() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("never-bound.sock");
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
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

    assert_two_failures(&mut failure_rx, iface, DialFailureKind::NotFound).await;
    assert!(connect_rx.try_recv().is_err(), "nothing connected while the path was unbound");

    let listener = UnixListener::bind(&socket).expect("bind the socket the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    while failure_rx.try_recv().is_ok() {}
    assert_silent("a dial failure after the connect", &mut failure_rx).await;

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

#[test]
fn dial_failure_is_copy_send_sync_and_the_state_event_stays_copy() {
    fn is_copy<T: Copy>() {}
    fn is_send_sync<T: Send + Sync>() {}
    is_copy::<DialFailure>();
    is_send_sync::<DialFailure>();
    is_copy::<DialFailureKind>();
    is_copy::<SharedInstanceEvent>();
    let event = SharedInstanceEvent::DialFailed { reason: DialFailureKind::Refused };
    let copied = event;
    assert_eq!(event, copied);
}
