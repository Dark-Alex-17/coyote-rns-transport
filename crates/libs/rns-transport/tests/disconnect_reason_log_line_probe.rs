//! The operator's view of a disconnect reason: both clients' `disconnected
//! from` info line names the stream exit (`exit=PeerClosed`,
//! `exit=ReadError(ConnectionReset)`, `exit=Cancelled`), so a reader of the
//! log can tell why a stream ended without a reason channel, and so the one
//! arm the reason channel is silent on, a stream ended because the
//! transport's receive queue closed, is still visible as `Cancelled`. The
//! records are captured through a `log::Log` installed once for the binary
//! and attributed to a test by the endpoint the line names. Each test reads
//! the reason channel beside the log so the line and the event are known to
//! describe the same drop.

use std::sync::{Mutex, Once};
use std::time::Duration;

use rns_transport::buffer::OutputBuffer;
use rns_transport::hash::AddressHash;
use rns_transport::iface::hdlc::Hdlc;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::packet::{Packet, PacketDataBuffer};
use rns_transport::serde::Serialize;
use rns_transport::transport::DisconnectReason;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::timeout;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const WITHIN: Duration = Duration::from_secs(2);
const SETTLE: Duration = Duration::from_millis(100);

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

/// Every captured info record whose message starts with `prefix`, in order.
fn info_lines_starting_with(prefix: &str) -> Vec<String> {
    CAPTURED
        .records
        .lock()
        .expect("captured record store")
        .iter()
        .filter(|(level, message)| *level == log::Level::Info && message.starts_with(prefix))
        .map(|(_, message)| message.clone())
        .collect()
}

/// Polls for `count` info lines starting with `prefix` within `WITHIN`: the
/// record is pushed before the reason is sent, so the poll is only a margin
/// for the arms that read no event first (the `Cancelled` lines).
async fn await_info_lines(prefix: &str, count: usize) -> Vec<String> {
    timeout(WITHIN, async {
        loop {
            let lines = info_lines_starting_with(prefix);
            if lines.len() >= count {
                return lines;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "waited for {count} info line(s) starting with {prefix:?}, have {:?}",
            info_lines_starting_with(prefix)
        )
    })
}

/// One Reticulum packet carrying `data`, framed for the HDLC stream the
/// clients read.
fn framed_packet(data: &[u8]) -> Vec<u8> {
    let packet = Packet { data: PacketDataBuffer::new_from_slice(data), ..Packet::default() };
    let mut bytes = vec![0_u8; 1024];
    let encoded_len = {
        let mut output = OutputBuffer::new(&mut bytes);
        packet.serialize(&mut output).expect("serialize packet");
        output.offset()
    };
    bytes.truncate(encoded_len);
    Hdlc::frame(&bytes).expect("frame packet")
}

#[deny(clippy::await_holding_invalid_type)]
async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(WITHIN, rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

#[deny(clippy::await_holding_invalid_type)]
async fn join_within(task: tokio::task::JoinHandle<()>, what: &str) {
    timeout(WITHIN, task)
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} panicked: {err}"));
}

/// A peer that closes, then one that resets, then the transport's receive
/// queue closing: three drops, three `disconnected from` lines, each naming
/// the exit the reason channel (or its silence) corresponds to.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_names_the_exit_on_its_disconnected_line() {
    install_capture_logger();
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let prefix = format!("disconnected from <{addr}>");
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    // An orderly close: the line names `PeerClosed`, the same word the
    // reason channel carries.
    let (first, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("first");
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );
    let lines = await_info_lines(&prefix, 1).await;
    assert_eq!(lines[0], format!("{prefix} exit=PeerClosed"), "the orderly close's line");

    // A reset on a held stream: the line names the io error kind the read
    // side saw; the event only says `Reset`.
    let (second, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("second");
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    socket2::SockRef::from(&second).set_linger(Some(Duration::ZERO)).expect("linger");
    drop(second);
    assert_eq!(recv_within("reset reason", &mut reason_rx).await, (iface, DisconnectReason::Reset));
    let lines = await_info_lines(&prefix, 2).await;
    assert!(
        lines[1].starts_with(&format!("{prefix} exit=")) && lines[1].contains("ConnectionReset"),
        "the reset's line names the io error kind, got {:?}",
        lines[1]
    );

    // The transport side goes away while the third stream is up: the reason
    // channel stays silent by contract, so the line is the only place that
    // says why the stream ended, and it says `Cancelled`.
    let (mut third, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("third");
    assert_eq!(recv_within("third connect event", &mut connect_rx).await, iface);
    drop(manager);
    third.write_all(&framed_packet(b"nowhere-to-deliver")).await.expect("write packet");
    let (fourth, _) = timeout(WITHIN, listener.accept())
        .await
        .expect("the client did not dial again after its receive queue closed")
        .expect("fourth");
    assert_eq!(recv_within("fourth connect event", &mut connect_rx).await, iface);
    let lines = await_info_lines(&prefix, 3).await;
    assert_eq!(lines[2], format!("{prefix} exit=Cancelled"), "the receive-queue-closed line");
    tokio::time::sleep(SETTLE).await;
    assert_eq!(
        reason_rx.try_recv(),
        Err(TryRecvError::Empty),
        "the reason channel is silent on the receive-queue-closed stream"
    );

    cancel.cancel();
    drop(listener);
    join_within(task, "TCP client task").await;
    drop(third);
    drop(fourth);
    // The cancel ends the fourth stream too: that one is logged as
    // `Cancelled` as well, and nothing else is logged under this endpoint.
    let lines = await_info_lines(&prefix, 4).await;
    assert_eq!(lines[3], format!("{prefix} exit=Cancelled"), "the cancelled stream's line");
    assert_eq!(lines.len(), 4, "one line per ended stream, no more");
}

/// The unix client's twin: an orderly close names `PeerClosed` on its own
/// `disconnected from local unix client` line.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_names_the_exit_on_its_disconnected_line() {
    install_capture_logger();
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("log-line.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    let (first, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("first");
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );
    let unix_lines = await_info_lines("disconnected from local unix client <", 1)
        .await
        .into_iter()
        .filter(|line| line.contains(&socket.display().to_string()))
        .collect::<Vec<_>>();
    assert_eq!(unix_lines.len(), 1, "one line for this socket, got {unix_lines:?}");
    assert!(
        unix_lines[0].ends_with("> exit=PeerClosed"),
        "the orderly close's line names PeerClosed, got {:?}",
        unix_lines[0]
    );

    let (second, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("second");
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);
    cancel.cancel();
    join_within(task, "unix client task").await;
    drop(second);
    drop(manager);
    assert_eq!(
        timeout(WITHIN, reason_rx.recv()).await.expect("reason channel stayed open"),
        None,
        "a cancelled client sends no reason"
    );
}
