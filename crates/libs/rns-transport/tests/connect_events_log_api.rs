//! The 0.17.0 CHANGELOG and the builders' rustdoc say "a full or closed
//! channel drops the connect event at debug". `tests/connect_events_api.rs`
//! pins that the client keeps serving through the drop; these pins hold the
//! level and the line: a consumer running at `info` sees nothing from a
//! dropped connect event, one debugging at `debug` sees one line per dropped
//! event naming the interface and the endpoint.
//!
//! Its own test target, since `log::set_logger` is process-wide and the
//! register pins in `tests/connect_events_api.rs` must not run under a
//! capturing logger. One `log::Log` is installed for the whole binary
//! (`log::set_logger` installs once), so records are attributed to a test by
//! the `endpoint=` they name: the TCP endpoint is an ephemeral
//! `127.0.0.1:<port>`, the unix endpoint a path under a fresh tempdir. The
//! interface address cannot be used for attribution:
//! `InterfaceManager::new_context` derives it from a per-manager counter, so
//! every test's first interface shares one address.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

struct CaptureLogger {
    records: Mutex<Vec<(log::Level, String, String)>>,
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
        self.records.lock().expect("captured record store").push((
            record.level(),
            record.target().to_string(),
            record.args().to_string(),
        ));
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

/// Every captured record whose message names `endpoint=<endpoint>`.
fn records_for_endpoint(endpoint: &str) -> Vec<(log::Level, String, String)> {
    let needle = format!("endpoint={endpoint}");
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, _, message)| message.contains(&needle))
        .cloned()
        .collect()
}

/// The records about a dropped *connect* event (not reconnect, not
/// disconnect) for one endpoint.
fn dropped_connect_records(endpoint: &str) -> Vec<(log::Level, String, String)> {
    records_for_endpoint(endpoint)
        .into_iter()
        .filter(|(_, _, message)| {
            message.starts_with("dropped ")
                && message.contains(" connect event iface=")
                && !message.contains("reconnect event")
                && !message.contains("disconnect event")
        })
        .collect()
}

async fn recv_within(what: &str, rx: &mut mpsc::Receiver<AddressHash>) -> AddressHash {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

fn assert_only_debug_drops(endpoint: &str, iface: AddressHash, expected_drops: usize) {
    let drops = dropped_connect_records(endpoint);
    assert_eq!(
        drops.len(),
        expected_drops,
        "one debug line per dropped connect event, got {drops:?}"
    );
    for (level, target, message) in &drops {
        assert_eq!(
            *level,
            log::Level::Debug,
            "the drop is logged at debug, not {level}: {message}"
        );
        assert!(
            message.contains(&format!("iface={iface}")),
            "the drop line names the interface address: {message}"
        );
        assert!(
            target.starts_with("rns_transport::iface::"),
            "the drop line comes from the client module, target {target}"
        );
    }
    let louder: Vec<_> = records_for_endpoint(endpoint)
        .into_iter()
        .filter(|(level, _, message)| {
            *level <= log::Level::Warn && message.contains("connect event")
        })
        .collect();
    assert!(
        louder.is_empty(),
        "a dropped connect event must never reach warn or error; got {louder:?}"
    );
}

/// TCP, capacity-one channel never read: the first connect buffers, the
/// second drops, and the drop is exactly one debug line naming iface and
/// endpoint.
#[tokio::test]
async fn tcp_client_logs_a_dropped_connect_event_at_debug_once_per_drop() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address").to_string();

    let (connect_tx, mut connect_rx) = mpsc::channel(1);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    let client = TcpClient::new(endpoint.clone())
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let first = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("first TCP connection timed out")
        .expect("first TCP connection failed")
        .0;
    assert!(
        dropped_connect_records(&endpoint).is_empty(),
        "the first connect event is buffered, nothing dropped yet"
    );
    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second TCP connection timed out")
        .expect("second TCP connection failed")
        .0;
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);
    assert_eq!(connect_rx.try_recv().ok(), Some(iface), "the first connect event is buffered");

    assert_only_debug_drops(&endpoint, iface, 1);

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// TCP, receiver dropped before the first stream: both connect events drop
/// (closed channel), each as one debug line; the second stream is still
/// accepted.
#[tokio::test]
async fn tcp_client_logs_each_connect_event_dropped_on_a_closed_channel_at_debug() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let endpoint = listener.local_addr().expect("listener address").to_string();

    let (connect_tx, connect_rx) = mpsc::channel(4);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    drop(connect_rx);
    let client = TcpClient::new(endpoint.clone())
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let first = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("first TCP connection timed out")
        .expect("first TCP connection failed")
        .0;
    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second TCP connection timed out")
        .expect("second TCP connection failed")
        .0;
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);

    assert_only_debug_drops(&endpoint, iface, 2);

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// Unix, capacity-one channel never read: the first connect buffers, the
/// second drops, and the drop is exactly one debug line naming iface and
/// endpoint.
#[cfg(unix)]
#[tokio::test]
async fn local_unix_client_logs_a_dropped_connect_event_at_debug_once_per_drop() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("connect-events-debug-full.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();

    let (connect_tx, mut connect_rx) = mpsc::channel(1);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);

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
    assert!(dropped_connect_records(&label).is_empty(), "nothing dropped on the first stream");
    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second unix connection timed out")
        .expect("second unix connection failed")
        .0;
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);
    assert_eq!(connect_rx.try_recv().ok(), Some(iface), "the first connect event is buffered");

    assert_only_debug_drops(&label, iface, 1);

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

/// Unix, receiver dropped before the first stream: both connect events drop
/// (closed channel), each as one debug line; the second stream is still
/// accepted.
#[cfg(unix)]
#[tokio::test]
async fn local_unix_client_logs_each_connect_event_dropped_on_a_closed_channel_at_debug() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("connect-events-debug-closed.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();

    let (connect_tx, connect_rx) = mpsc::channel(4);
    let (reconnect_tx, mut reconnect_rx) = mpsc::channel(4);
    drop(connect_rx);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(Duration::from_millis(50))
        .with_connect_events(connect_tx)
        .with_reconnect_events(reconnect_tx);

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
    drop(first);
    let second = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second unix connection timed out")
        .expect("second unix connection failed")
        .0;
    assert_eq!(recv_within("reconnect event", &mut reconnect_rx).await, iface);

    assert_only_debug_drops(&label, iface, 2);

    cancel.cancel();
    drop(second);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}
