//! Usage probes for the `TcpClient` give-up line and the count it leaves behind. The
//! CHANGELOG names the connect line's suffix ` after N failed attempt(s)`; the give-up
//! line a client logs once its `max_reconnect_tries` budget is spent uses the same
//! `failed attempt(s)` form, with N above the budget, so a consumer grepping its logs
//! matches both lines with one needle. The `Transport::interface_connect_count` rustdoc
//! adds: "A client that gave up after `max_reconnect_tries` keeps its last `Some(n)` until
//! the manager prunes its record."
//!
//! The first test gives up from a refused port and keeps `Some(0)` until `Transport::stop`
//! prunes it to `None`; the second gives up after one established stream and keeps the
//! `Some(1)` that stream earned until the same prune.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::iface::tcp_client::TcpClient;
use tokio::net::TcpListener;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/wait_until.rs"]
mod wait_until;
use client_transport::client_transport;
use free_loopback_port::free_loopback_port;
use wait_until::wait_until;

const WAIT: Duration = Duration::from_secs(5);
const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const GIVE_UP_PREFIX: &str = "max TCP reconnect attempts reached for <";
const ATTEMPTS_SUFFIX: &str = " failed attempt(s)";

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

/// Every captured message, of any level, that names `addr`.
fn lines_naming(addr: &str) -> Vec<String> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.contains(&format!("<{addr}>")))
        .map(|(_, message)| message.clone())
        .collect()
}

/// The give-up lines logged for `addr`.
fn give_up_lines(addr: &str) -> Vec<String> {
    lines_naming(addr).into_iter().filter(|line| line.starts_with(GIVE_UP_PREFIX)).collect()
}

/// Exactly one give-up line for `addr`, in the form
/// `max TCP reconnect attempts reached for <addr> after N failed attempt(s)` with N above
/// the budget of 1; and no line for `addr`, connect or give-up, falls back to the bare
/// plural `failed attempts`.
fn assert_give_up_line(addr: &str) -> u64 {
    let lines = give_up_lines(addr);
    assert_eq!(lines.len(), 1, "exactly one give-up line: {lines:?}");
    let line = &lines[0];
    let head = format!("{GIVE_UP_PREFIX}{addr}> after ");
    let count = line
        .strip_prefix(&head)
        .and_then(|tail| tail.strip_suffix(ATTEMPTS_SUFFIX))
        .unwrap_or_else(|| panic!("give-up line is `{head}N{ATTEMPTS_SUFFIX}`: {line:?}"));
    let count: u64 = count.parse().unwrap_or_else(|err| panic!("N in {line:?}: {err}"));
    assert!(count > 1, "the budget of 1 is spent once the count exceeds it: {line:?}");
    for message in lines_naming(addr) {
        assert!(
            !message.contains("failed attempts"),
            "every attempt count reads `failed attempt(s)`: {message:?}"
        );
    }
    count
}

/// A client that only ever reads `Connection refused` spends a budget of 1 on its second
/// attempt, says so in the `failed attempt(s)` form, and keeps `Some(0)` on the transport
/// until `stop` prunes it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn refused_client_gives_up_in_the_attempt_s_form_and_keeps_some_zero() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let transport = client_transport("give-up-refused");
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(1));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);
    assert_eq!(transport.interface_connect_count(&iface).await, Some(0));

    wait_until("the client gives up", || !give_up_lines(&addr).is_empty()).await;
    assert_give_up_line(&addr);
    wait_until("the client task closes its stream state", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;

    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(0),
        "a client that gave up keeps its last count until the manager prunes its record"
    );
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(0));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "pruned by stop");
}

/// A client whose first stream is accepted and dropped at once, against a listener that
/// then goes away, spends a budget of 1 on the refused redial; its give-up line uses the
/// `failed attempt(s)` form and the transport keeps the `Some(1)` the stream earned.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn client_that_connected_once_gives_up_and_keeps_some_one() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address").to_string();
    let transport = client_transport("give-up-after-one");
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(1));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    let (stream, _) = timeout(WAIT, listener.accept())
        .await
        .expect("the client dials within the deadline")
        .expect("accept");
    wait_until("the first stream is counted", || {
        status.to_json()["connected_streams"].as_u64() == Some(1)
    })
    .await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    drop(stream);
    drop(listener);

    wait_until("the client gives up", || !give_up_lines(&addr).is_empty()).await;
    assert_give_up_line(&addr);
    wait_until("the client task closes its stream state", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;

    assert_eq!(
        transport.interface_connect_count(&iface).await,
        Some(1),
        "a client that gave up keeps its last Some(n) until the manager prunes its record"
    );

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "pruned by stop");
}
