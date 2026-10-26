// Pins the pump's join discipline: the transmit loop is a spawned task that
// is JOINED, never aborted, so a write already in flight when the reader
// ends first still reaches the child and the receiver comes back for the
// respawn; and a writer that ends first (a write error, not a panic) cancels
// `stop` so a reader parked on an open pipe is collected instead of wedging
// the pump.

/// Handshake between the gated reader and the slow writer: the writer flips
/// `write_started` on its first poll and wakes the reader; the reader stays
/// pending until then and reports EOF afterwards.
struct WriteHandshake {
    write_started: std::sync::atomic::AtomicBool,
    reader_waker: std::sync::Mutex<Option<std::task::Waker>>,
}

impl WriteHandshake {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            write_started: std::sync::atomic::AtomicBool::new(false),
            reader_waker: std::sync::Mutex::new(None),
        })
    }
}

/// A child stdout that stays open until the writer has started a write, then
/// reports EOF so the reader ends while that write is still in flight.
struct EofOnceWriteStarts(Arc<WriteHandshake>);

impl tokio::io::AsyncRead for EofOnceWriteStarts {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        _: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.0.write_started.load(Ordering::SeqCst) {
            return std::task::Poll::Ready(Ok(()));
        }
        *self.0.reader_waker.lock().expect("reader waker slot") = Some(cx.waker().clone());
        std::task::Poll::Pending
    }
}

/// A child stdin whose first write takes `delay` to complete and counts the
/// bytes it accepted once it does.
struct SlowWriter {
    handshake: Arc<WriteHandshake>,
    delay: Duration,
    pending: Option<std::pin::Pin<Box<tokio::time::Sleep>>>,
    bytes: Arc<std::sync::atomic::AtomicUsize>,
    writes: Arc<std::sync::atomic::AtomicUsize>,
}

impl tokio::io::AsyncWrite for SlowWriter {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.pending.is_none() {
            let delay = self.delay;
            self.pending = Some(Box::pin(tokio::time::sleep(delay)));
            self.handshake.write_started.store(true, Ordering::SeqCst);
            if let Some(waker) =
                self.handshake.reader_waker.lock().expect("reader waker slot").take()
            {
                waker.wake();
            }
        }
        let sleep = self.pending.as_mut().expect("in-flight write timer");
        if std::future::Future::poll(sleep.as_mut(), cx).is_pending() {
            return std::task::Poll::Pending;
        }
        self.pending = None;
        self.bytes.fetch_add(buf.len(), Ordering::SeqCst);
        self.writes.fetch_add(1, Ordering::SeqCst);
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

// The reader reaches EOF while the writer is mid-write. A pump that aborts
// the writer instead of joining it drops the frame on the floor and reports
// the receiver lost (`transmit_task_failed`); the joined writer finishes the
// write and hands the receiver back.
#[tokio::test]
async fn pipe_stream_joins_an_in_flight_write_when_the_reader_ends_first() {
    let handshake = WriteHandshake::new();
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, tx_receiver) = mpsc::channel(8);
    tx_sender
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("queue tx message");
    let bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer = SlowWriter {
        handshake: handshake.clone(),
        delay: Duration::from_millis(150),
        pending: None,
        bytes: bytes.clone(),
        writes: writes.clone(),
    };
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    let mut tx_receiver = timeout(
        Duration::from_secs(2),
        super::run_pipe_stream(
            EofOnceWriteStarts(handshake),
            writer,
            crate::hash::AddressHash::new_from_slice(b"pipe-join-in-flight-write"),
            PipeInterface::DEFAULT_MTU,
            CancellationToken::new(),
            CancellationToken::new(),
            rx_channel,
            tx_receiver,
            runtime_status.clone(),
            ifac_state,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        ),
    )
    .await
    .expect("the pump returns once the joined writer finishes its write")
    .expect("a writer that exits normally hands the receiver back");

    assert_eq!(writes.load(Ordering::SeqCst), 1, "the in-flight write completed exactly once");
    assert!(bytes.load(Ordering::SeqCst) > 0, "the frame in flight at EOF reached the child");
    let status = runtime_status.lock().expect("pipe runtime status").clone();
    assert_eq!(status.process_state, "closed", "the reader's EOF is the recorded outcome");
    assert_eq!(status.last_error.as_deref(), Some("pipe stdout closed"));
    assert!(!status.pipe_is_open);
    tx_sender
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("the tx sender is still live after the pump returns");
    assert!(tx_receiver.try_recv().is_ok(), "the receiver is intact for the respawn");
}

/// A child stdin whose first write fails with a broken pipe.
struct BrokenPipeWriter;

impl tokio::io::AsyncWrite for BrokenPipeWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        _: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "injected pipe write error",
        )))
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

// A write error while the reader sits on an open pipe records the write-error
// status (`process_state == "write_error"`, `last_error` is the injected error,
// `pipe_is_open == false`) and hands the receiver back intact for the respawn.
// The writer's own `tx_stop.cancel()` releases the parked reader, not the pump.
#[tokio::test]
async fn pipe_stream_cancels_an_open_reader_after_a_write_error_and_keeps_the_receiver() {
    let (stream, _peer) = tokio::io::duplex(64);
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, tx_receiver) = mpsc::channel(8);
    tx_sender
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("queue tx message");
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    let mut tx_receiver = timeout(
        Duration::from_secs(1),
        super::run_pipe_stream(
            stream,
            BrokenPipeWriter,
            crate::hash::AddressHash::new_from_slice(b"pipe-write-error-open-reader"),
            PipeInterface::DEFAULT_MTU,
            CancellationToken::new(),
            CancellationToken::new(),
            rx_channel,
            tx_receiver,
            runtime_status.clone(),
            ifac_state,
            Arc::new(std::sync::atomic::AtomicU64::new(0)),
        ),
    )
    .await
    .expect("a writer exit cancels `stop` and the open reader is collected")
    .expect("a write error is not a panic: the receiver comes back");

    let status = runtime_status.lock().expect("pipe runtime status").clone();
    assert_eq!(status.process_state, "write_error");
    assert_eq!(status.last_error.as_deref(), Some("injected pipe write error"));
    assert!(!status.pipe_is_open);
    tx_sender
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
        .await
        .expect("the tx sender is still live after the pump returns");
    assert!(tx_receiver.try_recv().is_ok(), "the receiver is intact for the respawn");
}
