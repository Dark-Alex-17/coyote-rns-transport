//! Usage probes for the text the failed-dial cadence carries, written from
//! the contract: the failed line keeps its pre-cadence prefix byte-for-byte
//! and the error text that followed it, appending only ` (attempt N)`; the
//! TCP runtime status `last_error` is unchanged by the cadence.
//!
//! The committed suites pin the prefix, the numbering and that each line
//! `contains` the OS text, and they pin `last_error` by prefix and `contains`.
//! None of them compares the whole line or the whole `last_error` against the
//! text a caller can derive for itself, so a cadence that formats the error
//! another way (`{err:?}`), or that leaks its ` (attempt N)` suffix into the
//! status, survives every committed test. These probes build the expected
//! text from a plain `std` connect against the same endpoint and compare the
//! complete strings.

use std::sync::{Mutex, Once};
use std::time::Duration;

#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::transport::DialFailureKind;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/recv_failures.rs"]
mod recv_failures;
use free_loopback_port::free_loopback_port;
use recv_failures::recv_failures;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const CHANNEL_CAPACITY: usize = 128;

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

/// Every captured record of any level whose message starts with `prefix`.
fn lines_with_prefix(prefix: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.starts_with(prefix))
        .cloned()
        .collect()
}

/// Every line is `{prefix}{text} (attempt N)` with N running 1, 2, 3, ... in
/// log order: the pre-cadence line plus the suffix and nothing else.
fn assert_lines_are_prefix_text_and_attempt(
    lines: &[(log::Level, String)],
    prefix: &str,
    text: &str,
) {
    assert!(lines.len() >= 3, "at least three failed lines were logged: {lines:?}");
    for (index, (_, message)) in lines.iter().enumerate() {
        let attempt = index + 1;
        assert_eq!(
            *message,
            format!("{prefix}{text} (attempt {attempt})"),
            "line {attempt} is the pre-cadence line plus its attempt: {lines:?}"
        );
    }
}

/// A refused TCP dial logs exactly the OS text a plain connect to the same
/// closed port reports, and the runtime status `last_error` is exactly the
/// pre-cadence `tcp connect failed endpoint=<addr>: <text>` with no attempt
/// suffix, after warn-level and debug-level attempts alike.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_failed_lines_and_last_error_carry_the_bare_os_text() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let expected_text = std::net::TcpStream::connect(&addr)
        .expect_err("nothing listens on the freed port")
        .to_string();

    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);
    let status = client.runtime_status_handle();

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 3).await;
    let prefix = format!("couldn't connect to <{addr}>: ");
    let lines = lines_with_prefix(&prefix);
    assert_lines_are_prefix_text_and_attempt(&lines, &prefix, &expected_text);
    assert_eq!(lines[0].0, log::Level::Warn, "the first attempt warns: {lines:?}");
    assert!(
        lines[1..].iter().all(|(level, _)| *level == log::Level::Debug),
        "the attempts after the first debug: {lines:?}"
    );

    let status = status.to_json();
    let last_error = status["last_error"].as_str().expect("last_error is set after a failed dial");
    assert_eq!(
        last_error,
        format!("tcp connect failed endpoint={addr}: {expected_text}"),
        "last_error is the pre-cadence status text, without an attempt suffix"
    );
    assert!(
        status["reconnect_attempts"].as_u64().expect("reconnect_attempts is a number") >= 3,
        "every attempt is counted: {status}"
    );

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// A unix dial against a missing path logs exactly the OS text a plain
/// connect to the same path reports, after its unchanged prefix, plus the
/// attempt and nothing else.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_failed_lines_carry_the_bare_os_text() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("missing.sock");
    let expected_text = std::os::unix::net::UnixStream::connect(&socket)
        .expect_err("nothing is bound at the path")
        .to_string();
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();

    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 3).await;
    let prefix = format!("couldn't connect local unix client <{label}>: ");
    let lines = lines_with_prefix(&prefix);
    assert_lines_are_prefix_text_and_attempt(&lines, &prefix, &expected_text);
    assert_eq!(lines[0].0, log::Level::Warn, "the first attempt warns: {lines:?}");
    assert!(
        lines[1..].iter().all(|(level, _)| *level == log::Level::Debug),
        "the attempts after the first debug: {lines:?}"
    );

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}
