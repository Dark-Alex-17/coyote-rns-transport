// Pins the serial pump's join discipline: the transmit loop is a spawned task
// that is JOINED, never aborted, so a write already in flight when the port
// reports EOF still completes, is counted, and the receiver comes back for
// the reconnect.

use super::{run_serial_stream_with_ifac, SerialInterface, SerialStreamOptions};
use crate::hash::AddressHash;
use crate::iface::{TxMessage, TxMessageType};
use crate::packet::Packet;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

/// A port whose read half stays open until the first write has started, then
/// reports EOF, and whose first write takes `delay` to complete.
struct EofDuringSlowWritePort {
    write_started: Arc<std::sync::atomic::AtomicBool>,
    reader_waker: Arc<std::sync::Mutex<Option<std::task::Waker>>>,
    delay: Duration,
    pending: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
    bytes: Arc<std::sync::atomic::AtomicUsize>,
}

impl tokio::io::AsyncRead for EofDuringSlowWritePort {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.write_started.load(Ordering::SeqCst) {
            return std::task::Poll::Ready(Ok(()));
        }
        *self.reader_waker.lock().expect("reader waker slot") = Some(cx.waker().clone());
        std::task::Poll::Pending
    }
}

impl tokio::io::AsyncWrite for EofDuringSlowWritePort {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.pending.is_none() {
            let delay = self.delay;
            self.pending = Some(Box::pin(tokio::time::sleep(delay)));
            self.write_started.store(true, Ordering::SeqCst);
            if let Some(waker) = self.reader_waker.lock().expect("reader waker slot").take() {
                waker.wake();
            }
        }
        let sleep = self.pending.as_mut().expect("in-flight write timer");
        if std::future::Future::poll(sleep.as_mut(), cx).is_pending() {
            return std::task::Poll::Pending;
        }
        self.pending = None;
        self.bytes.fetch_add(buf.len(), Ordering::SeqCst);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}

// The port reports EOF while the writer is mid-write. The reader cancels
// `stop` and the pump collects the writer by JOINING it, so the frame is
// written and counted (`packets_tx`); an aborted writer would drop the frame
// and come back as a lost receiver with a `transmit task failed` error.
#[tokio::test]
async fn serial_stream_joins_an_in_flight_write_when_the_port_reports_eof_first() {
    let (rx_send, _rx_recv) = mpsc::channel(4);
    let (tx_send, tx_recv) = mpsc::channel(4);
    tx_send
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("queue tx message");
    let bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let port = EofDuringSlowWritePort {
        write_started: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        reader_waker: Arc::new(std::sync::Mutex::new(None)),
        delay: Duration::from_millis(150),
        pending: None,
        bytes: bytes.clone(),
    };
    let status = SerialInterface::new("eof-mid-write", 115_200).runtime_status_handle();

    let mut tx_recv = timeout(
        Duration::from_secs(2),
        run_serial_stream_with_ifac(
            port,
            SerialStreamOptions {
                iface_address: AddressHash::new_from_slice(b"serial-join-in-flight-write"),
                device: "eof-mid-write".to_string(),
                mtu: 512,
                cancel: CancellationToken::new(),
                rx_channel: rx_send,
                tx_channel: tx_recv,
                runtime_status: status.clone(),
            },
            Arc::new(std::sync::RwLock::new(None)),
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        ),
    )
    .await
    .expect("the pump returns once the joined writer finishes its write")
    .expect("a writer that exits normally hands the receiver back");

    assert!(bytes.load(Ordering::SeqCst) > 0, "the frame in flight at EOF reached the port");
    let status = status.snapshot();
    assert_eq!(status.packets_tx, 1, "the completed write is counted");
    assert_eq!(status.tx_errors, 0, "nothing about the join is a transmit error");
    assert_eq!(status.eof_count, 1, "the port's EOF is the recorded outcome");
    assert_eq!(status.link_state, "closed");
    assert_eq!(status.last_error, None, "no task failure is recorded");
    tx_send
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("the tx sender is still live after the pump returns");
    assert!(tx_recv.try_recv().is_ok(), "the receiver is intact for the reconnect");
}
