//! Usage probes for the `TcpClient` give-up line and the count it leaves behind. The
//! CHANGELOG names the connect line's suffix ` after N failed attempt(s)`; the give-up
//! line a client logs once its `max_reconnect_tries` budget is spent uses the same
//! `failed attempt(s)` form, with N above the budget, so a consumer grepping its logs
//! matches both lines with one needle. The `Transport::interface_connect_count` rustdoc
//! adds: "A client that gave up after `max_reconnect_tries` keeps its last `Some(n)` until
//! the manager prunes its record, which needs no call: giving up cancels the interface's
//! stop token, and the manager's next cleanup pass (every send, and the announce job's
//! one-second tick) drops cancelled records, so the read goes `None` within about a
//! second."
//!
//! Each test therefore reads the count right after the give-up line as `Some(n)` or
//! already `None`, polls it until it settles at `None` without calling `stop`, and checks
//! that `Transport::stop` leaves it there; the client's status handle outlives the record
//! and keeps `connected_streams` at `n` throughout. Three clients give up: one that only
//! ever reads `Connection refused` (n = 0; a budget of 1 spent on the second attempt), one
//! whose first stream is accepted and dropped at once before the listener goes away
//! (n = 1; a budget of 1 spent on the refused redial), and one with a budget of 0 whose
//! accepted and dropped stream is itself the one failed attempt (n = 1; the give-up line
//! reads `after 1 failed attempt(s)`).

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::hash::AddressHash;
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::transport::Transport;
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
const POLL: Duration = Duration::from_millis(20);
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
/// `budget`; and no line for `addr`, connect or give-up, falls back to the bare plural
/// `failed attempts`.
fn assert_give_up_line(addr: &str, budget: u64) -> u64 {
    let lines = give_up_lines(addr);
    assert_eq!(lines.len(), 1, "exactly one give-up line: {lines:?}");
    let line = &lines[0];
    let head = format!("{GIVE_UP_PREFIX}{addr}> after ");
    let count = line
        .strip_prefix(&head)
        .and_then(|tail| tail.strip_suffix(ATTEMPTS_SUFFIX))
        .unwrap_or_else(|| panic!("give-up line is `{head}N{ATTEMPTS_SUFFIX}`: {line:?}"));
    let count: u64 = count.parse().unwrap_or_else(|err| panic!("N in {line:?}: {err}"));
    assert!(count > budget, "the budget of {budget} is spent once the count exceeds it: {line:?}");
    for message in lines_naming(addr) {
        assert!(
            !message.contains("failed attempts"),
            "every attempt count reads `failed attempt(s)`: {message:?}"
        );
    }
    count
}

/// The count read right after the give-up line: still the client's last `Some(last)`, or
/// `None` if a cleanup pass already ran between the line and the read.
fn assert_last_count_or_pruned(count: Option<u64>, last: u64) {
    assert!(
        count.is_none() || count == Some(last),
        "a client that gave up reads its last Some({last}) until the manager's next cleanup \
         pass prunes its record: read {count:?}"
    );
}

/// Polls the transport until the record of a client that gave up is gone. Nothing in these
/// tests sends, so the pass that prunes it is the announce retransmit job's one-second
/// tick.
#[deny(clippy::await_holding_invalid_type)]
async fn wait_for_prune(transport: &Transport, iface: AddressHash) {
    timeout(WAIT, async {
        while transport.interface_connect_count(&iface).await.is_some() {
            tokio::time::sleep(POLL).await;
        }
    })
    .await
    .expect("the one-second announce retransmit job prunes the record of a client that gave up");
}

/// A client that only ever reads `Connection refused` spends a budget of 1 on its second
/// attempt, says so in the `failed attempt(s)` form, and reads `Some(0)` on the transport
/// until the manager's next cleanup pass prunes it to `None`.
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
    assert_give_up_line(&addr, 1);
    assert_last_count_or_pruned(transport.interface_connect_count(&iface).await, 0);
    wait_until("the client task closes its stream state", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(0));

    wait_for_prune(&transport, iface).await;
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "still pruned after stop");
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(0));
}

/// A client whose first stream is accepted and dropped at once, against a listener that
/// then goes away, spends a budget of 1 on the refused redial; its give-up line uses the
/// `failed attempt(s)` form and the transport reads the `Some(1)` the stream earned until
/// the next cleanup pass prunes it.
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

    let stream = accept_within(&listener, "the client's first dial").await;
    wait_until("the first stream is counted", || {
        status.to_json()["connected_streams"].as_u64() == Some(1)
    })
    .await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    drop(stream);
    drop(listener);

    wait_until("the client gives up", || !give_up_lines(&addr).is_empty()).await;
    assert_give_up_line(&addr, 1);
    assert_last_count_or_pruned(transport.interface_connect_count(&iface).await, 1);
    wait_until("the client task closes its stream state", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));

    wait_for_prune(&transport, iface).await;
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "still pruned after stop");
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));
}

/// A client with a budget of 0 whose only stream is accepted and dropped at once treats
/// that short-lived stream as its one failed attempt and gives up without redialing: the
/// give-up line reads `after 1 failed attempt(s)`, and the transport reads the `Some(1)`
/// the stream earned until the next cleanup pass prunes it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn short_lived_stream_spends_a_budget_of_zero_and_keeps_some_one() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address").to_string();
    let transport = client_transport("give-up-short-lived");
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_max_reconnect_tries(Some(0));
    let status = client.runtime_status_handle();
    let iface = transport.iface_manager().lock().await.spawn(client, TcpClient::spawn);

    let stream = accept_within(&listener, "the client's only dial").await;
    wait_until("the stream is counted", || {
        status.to_json()["connected_streams"].as_u64() == Some(1)
    })
    .await;
    assert_eq!(transport.interface_connect_count(&iface).await, Some(1));
    drop(stream);
    drop(listener);

    wait_until("the client gives up", || !give_up_lines(&addr).is_empty()).await;
    assert_eq!(assert_give_up_line(&addr, 0), 1, "the dropped stream is the one failed attempt");
    assert_last_count_or_pruned(transport.interface_connect_count(&iface).await, 1);
    wait_until("the client task closes its stream state", || {
        status.to_json()["stream_state"].as_str() == Some("closed")
    })
    .await;
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));

    wait_for_prune(&transport, iface).await;
    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
    assert_eq!(transport.interface_connect_count(&iface).await, None, "still pruned after stop");
    assert_eq!(status.to_json()["connected_streams"].as_u64(), Some(1));
}
