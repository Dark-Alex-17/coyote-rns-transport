//! Usage probes for the failed-dial log cadence, written from the contract
//! the README and CHANGELOG state rather than from the clients' code: the
//! failed-connect line warns on the first attempt since the last
//! established stream and logs the attempts between at debug, each ending
//! in ` (attempt N)`, where N is cleared at the success arm - so a flapping
//! peer re-arms the cadence at every connect - and the connect info line
//! names that count only when it is non-zero.
//!
//! The two re-arm probes drive a peer that accepts once, drops the stream at
//! once and goes away again (the unix twin also unlinks the socket file so the
//! renewed failures keep the `NotFound` kind and the second warn cannot be
//! the kind-change arm). Because the client publishes its dial-failure
//! item after each failed line and its connect event after the connect line
//! from one task, every failure that preceded a connect sits in the channel
//! by the time the connect event is read, so the count the connect line
//! names is pinned exactly, not as a floor. The two no-failure probes bind
//! the listener first and pin the bare `connected to <endpoint>` line
//! byte-for-byte. Each probe reads only its own endpoint's lines; the
//! capture store is shared across the binary.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::hash::AddressHash;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::transport::{DialFailure, DialFailureKind};
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::time::timeout;

#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
use free_loopback_port::free_loopback_port;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);
/// Comfortably above the dials a 50 ms cadence fits into one probe.
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

/// The `N` of a failed-dial line's trailing `(attempt N)`.
fn attempt_number(message: &str) -> u64 {
    let (_, tail) = message.rsplit_once(" (attempt ").expect("the line ends in its attempt");
    tail.strip_suffix(')')
        .expect("the attempt is parenthesised")
        .parse()
        .unwrap_or_else(|err| panic!("attempt number in {message:?}: {err}"))
}

/// The `N` of a connect line's ` after N failed attempt(s)` suffix.
fn failed_count(message: &str, prefix: &str) -> u64 {
    message
        .strip_prefix(prefix)
        .and_then(|tail| tail.strip_suffix(" failed attempt(s)"))
        .unwrap_or_else(|| panic!("the connect line ends in the failed count: {message}"))
        .parse()
        .unwrap_or_else(|err| panic!("failed count in {message:?}: {err}"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

/// `count` failures of `kind` on `iface`, one per attempt.
#[deny(clippy::await_holding_invalid_type)]
async fn recv_failures(
    rx: &mut mpsc::Receiver<DialFailure>,
    iface: AddressHash,
    kind: DialFailureKind,
    count: usize,
) {
    for attempt in 1..=count {
        let failure = recv_within(&format!("dial failure {attempt}"), rx).await;
        assert_eq!(failure.iface(), iface, "the failure names the interface");
        assert_eq!(failure.kind(), kind, "attempt {attempt}");
    }
}

/// Everything already queued on the dial-failure channel, all of `kind`.
fn drain_failures(rx: &mut mpsc::Receiver<DialFailure>, kind: DialFailureKind) -> u64 {
    let mut drained = 0;
    while let Ok(failure) = rx.try_recv() {
        assert_eq!(failure.kind(), kind, "a drained failure keeps its kind");
        drained += 1;
    }
    drained
}

/// The cadence a peer that comes up once, drops the stream at once and goes
/// away again produces: two warns, both numbered 1, with the line after each
/// warn at debug numbered 2, and each connect line naming exactly the
/// failures that preceded it (`before_first`, then `before_second`).
fn assert_rearmed_cadence(
    failed_lines: &[(log::Level, String)],
    connect_lines: &[(log::Level, String)],
    connect_prefix: &str,
    before_first: u64,
    before_second: u64,
) {
    for (_, message) in failed_lines {
        assert!(message.contains(" (attempt "), "every failed line is numbered: {message}");
    }
    let warns: Vec<usize> = failed_lines
        .iter()
        .enumerate()
        .filter(|(_, (level, _))| *level == log::Level::Warn)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(warns.len(), 2, "one warn per run of failures: {failed_lines:?}");
    for &warn in &warns {
        let (_, message) = &failed_lines[warn];
        assert_eq!(attempt_number(message), 1, "each warn opens a run at attempt 1: {message}");
        if let Some((level, next)) = failed_lines.get(warn + 1) {
            assert_eq!(*level, log::Level::Debug, "the attempt after a warn debugs: {next}");
            assert_eq!(attempt_number(next), 2, "the attempt after a warn is 2: {next}");
        }
    }
    let first_run = &failed_lines[..warns[1]];
    assert_eq!(
        first_run.len() as u64,
        before_first,
        "the first run logged one line per failure that preceded the first connect"
    );
    let second_run = &failed_lines[warns[1]..];
    assert_eq!(
        second_run.len() as u64,
        before_second,
        "the second run logged one line per failure that preceded the second connect"
    );

    assert_eq!(connect_lines.len(), 2, "two connect lines: {connect_lines:?}");
    for (level, message) in connect_lines {
        assert_eq!(*level, log::Level::Info, "the connect line is info: {message}");
    }
    assert_eq!(
        failed_count(&connect_lines[0].1, connect_prefix),
        before_first,
        "the first connect line names the failures before it: {connect_lines:?}"
    );
    assert_eq!(
        failed_count(&connect_lines[1].1, connect_prefix),
        before_second,
        "the second connect line names only the failures since the stream: {connect_lines:?}"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_rearms_the_warn_cadence_at_every_connect() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
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

    // First run: at least three refusals, then a listener that accepts once
    // and drops both the accepted stream and itself straight away.
    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 3).await;
    let listener = TcpListener::bind(&addr).await.expect("bind the port the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out")
        .expect("TCP connection failed")
        .0;
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    let before_first = 3 + drain_failures(&mut failure_rx, DialFailureKind::Refused);
    drop(listener);
    drop(stream);

    // Second run: the peer is gone again, so the dials fail with the same
    // kind; the cadence must open again at attempt 1.
    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 2).await;
    let listener = TcpListener::bind(&addr).await.expect("bind the port again");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second TCP connection timed out")
        .expect("second TCP connection failed")
        .0;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    let before_second = 2 + drain_failures(&mut failure_rx, DialFailureKind::Refused);

    let connect_prefix = format!("connected to <{addr}> after ");
    assert_rearmed_cadence(
        &lines_containing(&format!("couldn't connect to <{addr}>: ")),
        &lines_containing(&format!("connected to <{addr}>")),
        &connect_prefix,
        before_first,
        before_second,
    );

    cancel.cancel();
    drop(stream);
    drop(listener);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_rearms_the_warn_cadence_at_every_connect() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("flapping.sock");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();
    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 3).await;
    let listener = UnixListener::bind(&socket).expect("bind the socket the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed")
        .0;
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    let before_first = 3 + drain_failures(&mut failure_rx, DialFailureKind::NotFound);
    // Unlink the socket file before the stream goes, so the renewed failures
    // read `NotFound` again and a warn can only come from the re-armed count,
    // never from a change of kind.
    drop(listener);
    std::fs::remove_file(&socket).expect("unlink the socket file");
    drop(stream);

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 2).await;
    let listener = UnixListener::bind(&socket).expect("bind the socket again");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("second unix connection timed out")
        .expect("second unix connection failed")
        .0;
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    let before_second = 2 + drain_failures(&mut failure_rx, DialFailureKind::NotFound);

    let connect_prefix = format!("connected to local unix client <{label}> after ");
    assert_rearmed_cadence(
        &lines_containing(&format!("couldn't connect local unix client <{label}>: ")),
        &lines_containing(&format!("connected to local unix client <{label}>")),
        &connect_prefix,
        before_first,
        before_second,
    );

    cancel.cancel();
    drop(stream);
    drop(listener);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

/// A listener that is already up when the client starts: no failed line at
/// all, and the connect line is the pre-cadence `connected to <endpoint>`
/// byte-for-byte, with no ` after ...` suffix.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_connect_line_is_unchanged_when_the_first_dial_connects() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind a listener first");
    let addr = listener.local_addr().expect("listener address").to_string();
    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
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

    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out")
        .expect("TCP connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    assert!(failure_rx.try_recv().is_err(), "no dial failed");

    assert!(
        lines_containing(&format!("couldn't connect to <{addr}>: ")).is_empty(),
        "no failed-dial line for a dial that connected"
    );
    let connect_lines = lines_containing(&format!("connected to <{addr}>"));
    assert_eq!(
        connect_lines,
        vec![(log::Level::Info, format!("connected to <{addr}>"))],
        "the connect line carries no suffix when no dial failed"
    );

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_connect_line_is_unchanged_when_the_first_dial_connects() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("already-bound.sock");
    let listener = UnixListener::bind(&socket).expect("bind the socket first");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();
    let (failure_tx, mut failure_rx) = mpsc::channel(CHANNEL_CAPACITY);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    assert!(failure_rx.try_recv().is_err(), "no dial failed");

    assert!(
        lines_containing(&format!("couldn't connect local unix client <{label}>: ")).is_empty(),
        "no failed-dial line for a dial that connected"
    );
    let connect_lines = lines_containing(&format!("connected to local unix client <{label}>"));
    assert_eq!(
        connect_lines,
        vec![(log::Level::Info, format!("connected to local unix client <{label}>"))],
        "the connect line carries no suffix when no dial failed"
    );

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}
