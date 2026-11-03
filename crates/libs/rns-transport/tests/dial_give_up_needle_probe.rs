//! Usage probes for the two promises the give-up clause makes to a consumer reading a
//! `TcpClient`'s logs and its connect count: one needle, ` failed attempt(s)`, matches
//! both the connect info line and the give-up error line of the same client, and the
//! record of a client that gave up is pruned "within about a second" of the give-up line
//! with no call made, since the announce job's 1 s tick runs the manager's cleanup pass.
//!
//! The first test dials a port that refuses until a listener appears, so the connect
//! line carries a real count, then loses the listener and gives up: of every line the
//! client logs for its endpoint exactly two end in the needle, the info connect line and
//! the error give-up line, and the give-up count is the budget plus one because a stream
//! dropped before `MIN_STABLE_CONNECTION` is counted as a failed attempt without
//! resetting the budget. The second test stamps the give-up line and bounds the wait
//! until the transport reads `None`.

use std::sync::{Mutex, Once};
use std::time::{Duration, Instant};

use rns_transport::iface::tcp_client::TcpClient;
use tokio::net::TcpListener;
use tokio::time::timeout;

#[path = "support/accept_within.rs"]
mod accept_within;
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/wait_until.rs"]
mod wait_until;
use accept_within::accept_within;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// The announce job ticks every second; "within about a second" admits scheduling slack
/// on a loaded host but must stay well under the 5 s the sibling pins tolerate.
const ABOUT_A_SECOND: Duration = Duration::from_millis(2500);
const NEEDLE: &str = " failed attempt(s)";
const GIVE_UP_PREFIX: &str = "max TCP reconnect attempts reached for <";
const CONNECTED_PREFIX: &str = "connected to <";
const DIAL_FAILED_PREFIX: &str = "couldn't connect to <";

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

/// Every captured record, level and message, that names `addr`.
fn records_naming(addr: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.contains(&format!("<{addr}>")))
        .cloned()
        .collect()
}

fn count_with_prefix(addr: &str, prefix: &str) -> usize {
    records_naming(addr).iter().filter(|(_, m)| m.starts_with(prefix)).count()
}

/// The `N` before the needle at the end of `line`.
fn trailing_count(line: &str) -> u64 {
    let head = line.strip_suffix(NEEDLE).unwrap_or_else(|| panic!("ends in the needle: {line:?}"));
    let digits =
        head.rsplit(' ').next().unwrap_or_else(|| panic!("count before the needle: {line:?}"));
    digits.parse().unwrap_or_else(|err| panic!("N in {line:?}: {err}"))
}

/// A consumer who greps a client's log for ` failed attempt(s)` sees exactly two lines
/// once the client connected after failures and then gave up: the info connect line,
/// counting the refused dials before the stream, and the error give-up line, whose count
/// is the budget plus one.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn one_needle_matches_the_connect_line_and_the_give_up_line() {
    install_capture_logger();
    let budget = 6u64;
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let transport = client_transport("give-up-needle");
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(budget));
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));

    wait_until("two refused dials", || count_with_prefix(&addr, DIAL_FAILED_PREFIX) >= 2).await;
    let listener = TcpListener::bind(&addr).await.expect("bind the listener the client dials");
    let stream = accept_within(&listener, "the client's dial once the listener is up").await;
    wait_until("the connect line", || count_with_prefix(&addr, CONNECTED_PREFIX) == 1).await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    let refused_before_connect = count_with_prefix(&addr, DIAL_FAILED_PREFIX) as u64;
    assert!(
        refused_before_connect < budget,
        "the budget outlives the refusals: {refused_before_connect}"
    );

    drop(stream);
    drop(listener);
    wait_until("the client gives up", || count_with_prefix(&addr, GIVE_UP_PREFIX) == 1).await;

    let matches: Vec<(log::Level, String)> =
        records_naming(&addr).into_iter().filter(|(_, m)| m.ends_with(NEEDLE)).collect();
    assert_eq!(matches.len(), 2, "one needle, two lines (connect, give up): {matches:?}");
    let (connect_level, connect_line) = &matches[0];
    let (give_up_level, give_up_line) = &matches[1];
    assert_eq!(*connect_level, log::Level::Info, "connect line level: {connect_line:?}");
    assert!(
        connect_line.starts_with(&format!("{CONNECTED_PREFIX}{addr}> after ")),
        "{connect_line:?}"
    );
    assert_eq!(
        trailing_count(connect_line),
        refused_before_connect,
        "the connect line counts the refused dials before the stream: {connect_line:?}"
    );
    assert_eq!(*give_up_level, log::Level::Error, "give-up line level: {give_up_line:?}");
    assert!(
        give_up_line.starts_with(&format!("{GIVE_UP_PREFIX}{addr}> after ")),
        "{give_up_line:?}"
    );
    assert_eq!(
        trailing_count(give_up_line),
        budget + 1,
        "a short stream counts as a failed attempt without resetting the budget: {give_up_line:?}"
    );
    for (_, message) in records_naming(&addr) {
        assert!(
            !message.contains("failed attempts"),
            "no line falls back to the bare plural: {message:?}"
        );
    }

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None);
}

/// The record of a client that gave up is gone "within about a second" of its give-up
/// line with nothing sent and no `stop` called: the announce job's 1 s tick runs the
/// cleanup pass that drops it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn give_up_record_is_pruned_within_about_a_second_of_the_line() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let transport = client_transport("give-up-tick");
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(1));
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));

    wait_until("the client gives up", || count_with_prefix(&addr, GIVE_UP_PREFIX) == 1).await;
    let gave_up_at = Instant::now();
    let pruned_after = timeout(WAIT, async {
        while transport.interface_connect_count(&iface).await.is_some() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        gave_up_at.elapsed()
    })
    .await
    .expect("the record of a client that gave up is pruned without a call");
    assert!(
        pruned_after <= ABOUT_A_SECOND,
        "pruned {pruned_after:?} after the give-up line; the promise is within about a second"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None);
}
