//! Usage probes for what the failed-dial log cadence must NOT change,
//! written from the contract the CHANGELOG states: "`DialFailed` interface
//! events, the `with_dial_failure_events` channels and the TCP client's
//! `mark_reconnecting` / `last_error` still fire or record on every attempt;
//! only the log line's level moves."
//!
//! The committed suites pin the log lines and the events but never read the
//! TCP runtime status after a debug-level attempt: the one `reconnect_attempts
//! == 1` unit pin follows a single (warn-level) failure, and the `== 2` /
//! `== 3` pins exercise the short-stream arm, which records through a
//! different `mark_reconnecting` site. A regression that records the status
//! only on the warning attempts therefore survives every committed test.
//! These probes read the status a `rnstatus` consumer would read, after
//! several attempts of which exactly one warned.
//!
//! The second probe drives the `Timeout` arm (TEST-NET-1 is never routed,
//! as `tests/dial_failure_events_api.rs` already relies on) and pins that
//! the timed-out attempts follow the same cadence and numbering as refused
//! ones, each line carrying the connect timeout the README names.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::transport::DialFailureKind;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/attempt_number.rs"]
mod attempt_number;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/recv_failures.rs"]
mod recv_failures;
use attempt_number::attempt_number;
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

/// Every captured record of any level whose message contains `needle`.
fn lines_containing(needle: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.contains(needle))
        .cloned()
        .collect()
}

/// Exactly one warn, numbered 1, and at least `min_debugs` debug lines, every
/// line numbered and carrying `text`.
fn assert_cadence(lines: &[(log::Level, String)], min_debugs: usize, text: &str) {
    let warns: Vec<_> = lines.iter().filter(|(level, _)| *level == log::Level::Warn).collect();
    let debugs = lines.iter().filter(|(level, _)| *level == log::Level::Debug).count();
    assert_eq!(warns.len(), 1, "exactly one warn: {lines:?}");
    assert_eq!(attempt_number(&warns[0].1), 1, "the warn is attempt 1: {warns:?}");
    assert!(debugs >= min_debugs, "at least {min_debugs} debug lines: {lines:?}");
    for (_, message) in lines {
        assert!(message.contains(" (attempt "), "every line is numbered: {message}");
        assert!(message.contains(text), "every line carries {text:?}: {message}");
    }
}

/// The runtime status a `rnstatus` reader sees must count every failed
/// attempt and carry the last attempt's error text, whether that attempt
/// logged at warn or at debug.
fn assert_status_counts_every_attempt(
    status: &serde_json::Value,
    addr: &str,
    lines: &[(log::Level, String)],
    min_attempts: u64,
    text: &str,
) {
    let reconnect_attempts =
        status["reconnect_attempts"].as_u64().expect("reconnect_attempts is a number");
    assert!(
        reconnect_attempts >= min_attempts,
        "every failed attempt bumps reconnect_attempts, not only the warned ones: \
         reconnect_attempts={reconnect_attempts} lines={lines:?}"
    );
    // The status is marked before the line is logged, so it can never trail
    // the lines; it may lead them by the one attempt in flight.
    assert!(
        reconnect_attempts >= lines.len() as u64,
        "reconnect_attempts={reconnect_attempts} trails the {} logged lines",
        lines.len()
    );
    assert_eq!(status["stream_state"].as_str(), Some("reconnecting"), "{status}");
    let last_error =
        status["last_error"].as_str().expect("last_error is set after a debug attempt");
    let prefix = format!("tcp connect failed endpoint={addr}: ");
    assert!(last_error.starts_with(&prefix), "last_error keeps its prefix: {last_error}");
    assert!(last_error.contains(text), "last_error carries the error text: {last_error}");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_runtime_status_counts_the_debug_level_attempts_too() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
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

    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 4).await;
    let lines = lines_containing(&format!("couldn't connect to <{addr}>: "));
    assert_cadence(&lines, 3, "Connection refused");
    assert_status_counts_every_attempt(&status.to_json(), &addr, &lines, 4, "Connection refused");

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// 192.0.2.1 (TEST-NET-1) is never routed, so only the connect timeout ends
/// each dial; the timed-out attempts follow the same cadence as refused ones.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_timed_out_dials_follow_the_same_cadence_and_count() {
    install_capture_logger();
    let addr = "192.0.2.1:2";
    let connect_timeout = Duration::from_millis(200);
    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let client = TcpClient::new(addr)
        .with_connect_timeout(connect_timeout)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);
    let status = client.runtime_status_handle();

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::Timeout, 3).await;
    let text = format!("connect timed out after {connect_timeout:?}");
    let lines = lines_containing(&format!("couldn't connect to <{addr}>: "));
    assert_cadence(&lines, 2, &text);
    assert_status_counts_every_attempt(&status.to_json(), addr, &lines, 3, &text);

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}
