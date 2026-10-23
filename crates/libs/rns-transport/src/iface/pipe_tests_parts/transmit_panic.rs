// Panic handling in the pipe pump: a writer panic is caught whichever poll
// raises it, a payload that is not a string still yields a recorded error,
// the frames written before the panic are not lost, and a reader panic stops
// an idle writer instead of leaving the pump waiting on it.

/// A child stdin that accepts `accept_writes` whole writes, counting the
/// bytes, and panics on the next one with the payload `panic_with` builds.
struct LaterPanickingWriter {
    accept_writes: usize,
    writes: Arc<std::sync::atomic::AtomicUsize>,
    bytes: Arc<std::sync::atomic::AtomicUsize>,
    panic_with: fn() -> !,
}

impl tokio::io::AsyncWrite for LaterPanickingWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.writes.fetch_add(1, Ordering::SeqCst) < self.accept_writes {
            self.bytes.fetch_add(buf.len(), Ordering::SeqCst);
            return std::task::Poll::Ready(Ok(buf.len()));
        }
        (self.panic_with)()
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

/// Runs the pipe pump with `queued` broadcasts waiting and a writer that
/// panics after `accept_writes` writes; returns the receiver, the status and
/// the bytes the writer accepted.
async fn run_pipe_pump_until_writer_panics(
    queued: usize,
    accept_writes: usize,
    panic_with: fn() -> !,
) -> (mpsc::Receiver<TxMessage>, super::PipeRuntimeStatus, usize) {
    let (stream, _peer) = tokio::io::duplex(64);
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, mut tx_receiver) = mpsc::channel(8);
    for _ in 0..queued {
        tx_sender
            .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
            .await
            .expect("queue tx message");
    }
    let bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer = LaterPanickingWriter {
        accept_writes,
        writes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        bytes: bytes.clone(),
        panic_with,
    };
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    timeout(
        Duration::from_secs(1),
        super::run_pipe_stream(
            stream,
            writer,
            crate::hash::AddressHash::new_from_slice(b"pipe-tx-later-panic"),
            PipeInterface::DEFAULT_MTU,
            CancellationToken::new(),
            CancellationToken::new(),
            rx_channel,
            &mut tx_receiver,
            runtime_status.clone(),
            ifac_state,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        ),
    )
    .await
    .expect("a transmit loop panic returns from the driver instead of unwinding it");

    let status = runtime_status.lock().expect("pipe runtime status").clone();
    (tx_receiver, status, bytes.load(Ordering::SeqCst))
}

#[tokio::test]
async fn pipe_stream_panic_on_a_later_write_keeps_the_rest_of_the_queue() {
    let (mut tx_receiver, status, bytes_written) =
        run_pipe_pump_until_writer_panics(3, 1, || panic!("injected later pipe write panic")).await;

    assert!(bytes_written > 0, "the frame written before the panic reached the child");
    assert!(
        tx_receiver.try_recv().is_ok(),
        "the receiver survives the panic with the messages the loop never took"
    );
    assert_eq!(status.process_state, "transmit_task_failed");
    assert!(!status.pipe_is_open);
    assert_eq!(status.last_error.as_deref(), Some("injected later pipe write panic"));
}

#[tokio::test]
async fn pipe_stream_records_a_non_string_panic_payload_as_a_panic() {
    let (mut tx_receiver, status, _) =
        run_pipe_pump_until_writer_panics(2, 0, || std::panic::panic_any(7u32)).await;

    assert!(tx_receiver.try_recv().is_ok(), "the receiver survives a non-string panic");
    assert_eq!(status.process_state, "transmit_task_failed");
    assert!(!status.pipe_is_open);
    assert_eq!(
        status.last_error.as_deref(),
        Some("panic (non-string payload)"),
        "a payload that is neither &str nor String is still recorded"
    );
}

#[tokio::test]
async fn pipe_stream_panic_with_an_owned_message_is_recorded_verbatim() {
    let (_, status, _) =
        run_pipe_pump_until_writer_panics(1, 0, || panic!("owned {}", "pipe message")).await;

    assert_eq!(status.process_state, "transmit_task_failed");
    assert_eq!(status.last_error.as_deref(), Some("owned pipe message"));
}

/// A child stdout whose first read panics.
struct PanickingReader;

impl tokio::io::AsyncRead for PanickingReader {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        panic!("injected pipe read panic")
    }
}

// The writer has nothing queued and no cancel, so it only exits on `stop`;
// a pump that awaits the writer before joining the reader never returns.
#[tokio::test]
async fn pipe_stream_returns_after_a_receive_task_panic_with_the_writer_idle() {
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, mut tx_receiver) = mpsc::channel(8);
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    timeout(
        Duration::from_secs(1),
        super::run_pipe_stream(
            PanickingReader,
            tokio::io::sink(),
            crate::hash::AddressHash::new_from_slice(b"pipe-rx-panic"),
            PipeInterface::DEFAULT_MTU,
            CancellationToken::new(),
            CancellationToken::new(),
            rx_channel,
            &mut tx_receiver,
            runtime_status.clone(),
            ifac_state,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        ),
    )
    .await
    .expect("a receive task panic stops the writer and returns from the driver");

    let status = runtime_status.lock().expect("pipe runtime status").clone();
    assert_eq!(status.process_state, "receive_task_failed");
    assert!(!status.pipe_is_open);
    assert!(
        status.last_error.as_deref().is_some_and(|e| e.contains("injected pipe read panic")),
        "the join error carries the reader's panic message: {:?}",
        status.last_error
    );
    tx_sender
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("the tx sender is still live after the pump returns");
    assert!(tx_receiver.try_recv().is_ok(), "the receiver is intact for the respawn");
}
