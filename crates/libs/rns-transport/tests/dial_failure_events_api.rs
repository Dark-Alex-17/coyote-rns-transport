//! The README's dial-failure surface used from outside the crate:
//! `TcpClient::with_dial_failure_events` and
//! `LocalUnixClient::with_dial_failure_events` report every failed connect
//! attempt as a `DialFailure` naming the interface and a `DialFailureKind`,
//! `Refused` for a TCP port nothing listens on and `NotFound` for a unix
//! socket path nothing has bound, one per attempt until the socket appears,
//! after which the connect event fires and the dial-failure channel goes
//! quiet. The TCP pin also reads, through a capturing `log::Log` installed
//! once for the binary, that each failed attempt logs the endpoint and the
//! io error's own text, "Connection refused" for a closed port, so a reader
//! of the log can tell a refusal from a timeout; the records are attributed
//! by the endpoint they name. Only the first failed attempt since the last
//! established stream, a change of failure kind and every 30th attempt
//! warn; the attempts between log at debug, each line ending in its
//! `(attempt N)`, and the connect line names how many failed attempts it
//! took. A TCP client left dialing a closed port warns again on attempt 30
//! with attempts 2 to 29 at debug. A dial against the black-holed TEST-NET-1
//! address that outlives `with_connect_timeout` reports `Timeout` and warns
//! "connect timed out after" the configured timeout, the arm no io error
//! reaches. The README's `Io(kind)` arm is read through the unix client: a
//! bound socket inside a directory stripped of search permission fails each
//! dial with the OS's `Permission denied`, reported as `Io(PermissionDenied)`
//! and never one of the three named kinds, with the unix client's warn
//! carrying the io error's own text; restoring the permission lets the next
//! attempt connect and the channel goes quiet. The TCP twin of that arm has
//! no black-box fixture; its classification is pinned in-crate by
//! `classify_dial_error`. Both clients answer `dial_failure_events_enabled()`
//! false until the builder sets the channel. The last pin holds at compile
//! time that `DialFailure` is `Copy + Send + Sync` and that
//! `SharedInstanceEvent` stayed `Copy` with its new `DialFailed` variant.

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};
use std::sync::{Mutex, Once};
use std::time::Duration;

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

#[path = "support/attempt_number.rs"]
mod attempt_number;
#[path = "support/free_loopback_port.rs"]
mod free_loopback_port;
#[path = "support/recv_failures.rs"]
mod recv_failures;
#[path = "support/recv_within.rs"]
mod recv_within;
use attempt_number::attempt_number;
use free_loopback_port::free_loopback_port;
use recv_failures::recv_failures;
use recv_within::recv_within;

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

/// Every captured record of any level whose message contains `needle`.
fn dial_lines_containing(needle: &str) -> Vec<(log::Level, String)> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(_, message)| message.contains(needle))
        .cloned()
        .collect()
}

/// Every captured warn whose message contains `needle`.
fn warns_containing(needle: &str) -> Vec<String> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(level, message)| *level == log::Level::Warn && message.contains(needle))
        .map(|(_, message)| message.clone())
        .collect()
}

/// Every captured record carrying the TCP client's failed-dial line for `endpoint`.
fn failed_dial_lines(endpoint: &str) -> Vec<(log::Level, String)> {
    dial_lines_containing(&format!("couldn't connect to <{endpoint}>: "))
}

/// Every captured warn carrying the TCP client's failed-dial line for `endpoint`.
fn failed_dial_warns(endpoint: &str) -> Vec<String> {
    warns_containing(&format!("couldn't connect to <{endpoint}>: "))
}

/// Every captured record carrying the unix client's failed-dial line for
/// the endpoint labelled `label`; the unix tests share the capture store, so
/// each one reads only its own endpoint.
#[cfg(unix)]
fn failed_unix_dial_lines(label: &str) -> Vec<(log::Level, String)> {
    dial_lines_containing(&format!("couldn't connect local unix client <{label}>: "))
}

/// Every captured warn carrying the unix client's failed-dial line.
#[cfg(unix)]
fn failed_unix_dial_warns() -> Vec<String> {
    warns_containing("couldn't connect local unix client <")
}

/// Exactly one warn and at least `min_debugs` debug lines, every one of
/// them numbered, the warn being attempt 1 and the numbers covering
/// 1..=`min_debugs + 1`: the cadence a peer that stays down produces until
/// the stride is reached.
fn assert_first_warn_then_debugs(lines: &[(log::Level, String)], min_debugs: usize) {
    let warns: Vec<_> = lines.iter().filter(|(level, _)| *level == log::Level::Warn).collect();
    let debugs = lines.iter().filter(|(level, _)| *level == log::Level::Debug).count();
    assert_eq!(warns.len(), 1, "one warn for the first failed attempt: {lines:?}");
    assert!(debugs >= min_debugs, "the attempts after the first debug: {lines:?}");
    for (_, message) in lines {
        assert!(message.contains(" (attempt "), "every line is numbered: {message}");
    }
    assert_eq!(attempt_number(&warns[0].1), 1, "the warn is the first attempt: {warns:?}");
    for expected in 1..=(min_debugs as u64 + 1) {
        assert!(
            lines.iter().any(|(_, message)| attempt_number(message) == expected),
            "attempt {expected} was logged: {lines:?}"
        );
    }
}

/// Exactly one info line opens with `prefix` and names at least
/// `min_failed` failed attempts; dials between the last failure a test
/// read and its bind are allowed, so the count is a floor.
fn assert_connected_after_failed_attempts(prefix: &str, min_failed: u64) {
    let lines = dial_lines_containing(prefix);
    assert_eq!(lines.len(), 1, "one connect line names the failed attempts: {lines:?}");
    let (level, message) = &lines[0];
    assert_eq!(*level, log::Level::Info, "the connect line is info: {message}");
    let count = message
        .strip_prefix(prefix)
        .and_then(|tail| tail.strip_suffix(" failed attempt(s)"))
        .unwrap_or_else(|| panic!("the connect line ends in the failed count: {message}"))
        .parse::<u64>()
        .unwrap_or_else(|err| panic!("failed count in {message:?}: {err}"));
    assert!(count >= min_failed, "at least {min_failed} failed attempts were counted: {message}");
}

/// Puts the directory's search permission back when dropped, so a failing
/// assertion still lets the tempdir clean itself up.
#[cfg(unix)]
struct RestorePermissions(PathBuf);

#[cfg(unix)]
impl Drop for RestorePermissions {
    fn drop(&mut self) {
        let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
    }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod the socket dir");
}

/// Once the connect has fired, the dial-failure channel must stay quiet; a
/// grace window of several reconnect waits is the only witness an absence
/// test can have.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_silent<T>(what: &str, rx: &mut mpsc::Receiver<T>) {
    assert!(
        timeout(Duration::from_millis(300), rx.recv()).await.is_err(),
        "{what} should not have fired"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_reports_refused_dials_until_a_listener_binds_the_port() {
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx);
    assert!(!client.dial_failure_events_enabled(), "no dial-failure channel by default");
    let client = client.with_dial_failure_events(failure_tx);
    assert!(client.dial_failure_events_enabled(), "the builder sets the channel");

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 2).await;
    assert!(connect_rx.try_recv().is_err(), "nothing connected while the port was closed");
    let lines = failed_dial_lines(&addr);
    let warns = lines.iter().filter(|(level, _)| *level == log::Level::Warn).count();
    let debugs = lines.iter().filter(|(level, _)| *level == log::Level::Debug).count();
    assert_eq!(warns, 1, "the first failed attempt warns, the next one debugs: {lines:?}");
    assert!(debugs >= 1, "the second failed attempt debugs: {lines:?}");
    for (level, message) in &lines {
        assert!(
            message.contains("Connection refused"),
            "the line carries the io error's text: {message}"
        );
        assert!(message.contains(" (attempt "), "the line is numbered: {message}");
        if *level == log::Level::Warn {
            assert!(message.ends_with(" (attempt 1)"), "the warn is attempt 1: {message}");
        }
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_warns_once_then_debugs_until_a_connect_names_the_failed_attempts() {
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

    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, 4).await;
    let lines = failed_dial_lines(&addr);
    let prefix = format!("couldn't connect to <{addr}>: ");
    for (_, message) in &lines {
        assert!(message.starts_with(&prefix), "the prefix is unchanged: {message}");
    }
    assert_first_warn_then_debugs(&lines, 3);

    let listener = TcpListener::bind(&addr).await.expect("bind the port the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("TCP connection timed out")
        .expect("TCP connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    assert_connected_after_failed_attempts(&format!("connected to <{addr}> after "), 4);
    while failure_rx.try_recv().is_ok() {}
    assert_silent("a dial failure after the connect", &mut failure_rx).await;

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// The black-box pin of the documented "every 30th attempt" arm, the one
/// the unit tests in `dial_log.rs` pin in-crate: a TCP client left dialing
/// a closed port warns on attempt 1, debugs through attempt 29, warns again
/// on attempt 30 and debugs after it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_warns_again_on_the_thirtieth_failed_attempt() {
    /// Mirrors `DIAL_WARN_STRIDE` in `src/iface/dial_log.rs`, which an
    /// integration test cannot name; the reticulumd docs contract pins the
    /// documented figure to that source literal.
    const STRIDE: u64 = 30;
    install_capture_logger();
    let port = free_loopback_port().await;
    let addr = format!("127.0.0.1:{port}");
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let client = TcpClient::new(addr.clone())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::Refused, STRIDE as usize).await;
    let lines = failed_dial_lines(&addr);
    for (_, message) in &lines {
        assert!(message.contains(" (attempt "), "every line is numbered: {message}");
    }
    let level_of = |attempt: u64| -> Option<log::Level> {
        lines
            .iter()
            .find(|(_, message)| attempt_number(message) == attempt)
            .map(|(level, _)| *level)
    };
    assert_eq!(level_of(1), Some(log::Level::Warn), "attempt 1 warns: {lines:?}");
    assert_eq!(level_of(STRIDE), Some(log::Level::Warn), "attempt {STRIDE} warns again: {lines:?}");
    for attempt in 2..STRIDE {
        assert_eq!(
            level_of(attempt),
            Some(log::Level::Debug),
            "attempt {attempt} debugs before the stride: {lines:?}"
        );
    }
    for (level, message) in &lines {
        let attempt = attempt_number(message);
        if attempt > STRIDE && attempt < 2 * STRIDE {
            assert_eq!(
                *level,
                log::Level::Debug,
                "attempt {attempt} debugs after the stride: {message}"
            );
        }
    }

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

/// 192.0.2.1 (TEST-NET-1) is never routed, so the SYN gets no answer and
/// only the connect timeout ends the dial.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_reports_a_black_holed_dial_as_a_timeout_naming_the_connect_timeout() {
    install_capture_logger();
    let addr = "192.0.2.1:1";
    let connect_timeout = Duration::from_millis(200);
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let client = TcpClient::new(addr)
        .with_connect_timeout(connect_timeout)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    let failure = recv_within("timed-out dial failure", &mut failure_rx).await;
    assert_eq!(failure.iface(), iface, "the failure names the interface");
    assert_eq!(failure.kind(), DialFailureKind::Timeout);
    let warns = failed_dial_warns(addr);
    let expected = format!("connect timed out after {connect_timeout:?}");
    assert!(
        warns.iter().any(|message| message.contains(&expected)),
        "the warn names the connect timeout: {warns:?}"
    );

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task timed out")
        .expect("TCP client task panicked");
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_reports_not_found_dials_until_the_socket_is_bound() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("never-bound.sock");
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let (connect_tx, mut connect_rx) = mpsc::channel(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx);
    assert!(!client.dial_failure_events_enabled(), "no dial-failure channel by default");
    let client = client.with_dial_failure_events(failure_tx);
    assert!(client.dial_failure_events_enabled(), "the builder sets the channel");

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 2).await;
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

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_warns_once_then_debugs_until_a_connect_names_the_failed_attempts() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("bound-later.sock");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
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

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 4).await;
    let lines = failed_unix_dial_lines(&label);
    let prefix = format!("couldn't connect local unix client <{label}>: ");
    for (_, message) in &lines {
        assert!(message.starts_with(&prefix), "the prefix is unchanged: {message}");
    }
    assert_first_warn_then_debugs(&lines, 3);

    let listener = UnixListener::bind(&socket).expect("bind the socket the client dials");
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out")
        .expect("unix connection failed")
        .0;
    assert_eq!(recv_within("connect event", &mut connect_rx).await, iface);
    assert_connected_after_failed_attempts(
        &format!("connected to local unix client <{label}> after "),
        4,
    );
    while failure_rx.try_recv().is_ok() {}
    assert_silent("a dial failure after the connect", &mut failure_rx).await;

    cancel.cancel();
    drop(stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

/// A missing path reads `NotFound`; a stale socket file put there reads
/// `Refused`. The change of kind is the second warn, however many debug
/// attempts lie between.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_warns_again_when_the_failure_kind_changes() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("missing-then-stale.sock");
    let endpoint = LocalUnixEndpoint::filesystem(&socket);
    let label = endpoint.label();
    let (failure_tx, mut failure_rx) = mpsc::channel(8);
    let client = LocalUnixClient::new_connect(endpoint)
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_dial_failure_events(failure_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    recv_failures(&mut failure_rx, iface, DialFailureKind::NotFound, 2).await;
    // Bind and drop a listener on a sibling path, then rename it into place:
    // tokio's UnixListener does not unlink the file on drop, so the stale
    // path remains, and the atomic rename keeps the client out of the
    // listener's own bind-to-close window, where a dial would succeed into
    // the backlog and clear the attempt count.
    let staging = dir.path().join("staging.sock");
    drop(UnixListener::bind(&staging).expect("bind then abandon the staging socket"));
    fs::rename(&staging, &socket).expect("move the stale socket file into place");
    assert!(socket.exists(), "the stale socket file is the fixture");
    // The OS's own text for the refusal the client is about to meet.
    let refused_text = std::os::unix::net::UnixStream::connect(&socket)
        .expect_err("nothing accepts on the stale socket")
        .to_string();

    let mut refused = false;
    for attempt in 1..=10 {
        let failure =
            recv_within(&format!("dial failure after the bind {attempt}"), &mut failure_rx).await;
        if failure.kind() == DialFailureKind::Refused {
            refused = true;
            break;
        }
        assert_eq!(failure.kind(), DialFailureKind::NotFound, "attempt {attempt}");
    }
    assert!(refused, "the stale socket file turned the failure into Refused");

    let lines = failed_unix_dial_lines(&label);
    for (_, message) in &lines {
        assert!(message.contains(" (attempt "), "every line is numbered: {message}");
    }
    let warns: Vec<_> =
        lines.iter().enumerate().filter(|(_, (level, _))| *level == log::Level::Warn).collect();
    assert_eq!(warns.len(), 2, "the first attempt and the change of kind warn: {lines:?}");
    let (first_warn, (_, first_message)) = warns[0];
    let (second_warn, (_, second_message)) = warns[1];
    assert_eq!(attempt_number(first_message), 1, "the first warn is attempt 1: {first_message}");
    let first_refused = lines
        .iter()
        .position(|(_, message)| message.contains(&refused_text))
        .unwrap_or_else(|| panic!("a line carries {refused_text:?}: {lines:?}"));
    assert_eq!(second_warn, first_refused, "the second warn is the first refused line: {lines:?}");
    assert!(
        attempt_number(second_message) > 1,
        "the kind changed after attempt 1: {second_message}"
    );
    assert!(
        lines[first_warn + 1..second_warn].iter().all(|(level, _)| *level == log::Level::Debug),
        "every line between the two warns is debug: {lines:?}"
    );

    cancel.cancel();
    timeout(Duration::from_secs(2), task)
        .await
        .expect("unix client task timed out")
        .expect("unix client task panicked");
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_reports_an_unreachable_socket_as_io_permission_denied_quoting_the_error()
{
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let locked = dir.path().join("locked");
    fs::create_dir(&locked).expect("create the locked dir");
    let socket = locked.join("shared.sock");
    let listener = UnixListener::bind(&socket).expect("bind the socket the client dials");
    set_mode(&locked, 0o000);
    let _restore = RestorePermissions(locked.clone());
    // The OS's own text for the refusal the client is about to meet, taken
    // from the same locked directory so the oracle follows the platform.
    let os_error = match fs::read_dir(&locked) {
        Err(err) => err,
        Ok(_) => {
            eprintln!("skipping: this user can search a mode-000 directory (root?)");
            return;
        }
    };
    assert_eq!(os_error.kind(), std::io::ErrorKind::PermissionDenied, "fixture: {os_error}");

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

    recv_failures(
        &mut failure_rx,
        iface,
        DialFailureKind::Io(std::io::ErrorKind::PermissionDenied),
        2,
    )
    .await;
    assert!(connect_rx.try_recv().is_err(), "nothing connected while the dir was locked");

    let expected = os_error.to_string();
    let warns = failed_unix_dial_warns();
    assert!(
        warns.iter().any(|message| message.contains(&expected)),
        "the unix client's warn carries the io error's text {expected:?}: {warns:?}"
    );

    set_mode(&locked, 0o700);
    let stream = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("unix connection timed out once the dir was readable again")
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
