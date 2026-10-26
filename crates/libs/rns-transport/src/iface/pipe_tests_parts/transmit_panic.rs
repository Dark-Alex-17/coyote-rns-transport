// Panic handling in the pipe pump: a writer panic on a later write keeps the
// frames already written, records the failure and reports the receiver lost,
// and a reader panic stops an idle writer instead of leaving the pump waiting
// on it.

/// A child stdin that accepts `accept_writes` whole writes, counting the
/// bytes, and panics on the next one.
struct LaterPanickingWriter {
    accept_writes: usize,
    writes: Arc<std::sync::atomic::AtomicUsize>,
    bytes: Arc<std::sync::atomic::AtomicUsize>,
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
        panic!("injected later pipe write panic")
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

// The reader sits on an open duplex and only exits on `stop`; a pump that
// joins the panicked writer without cancelling `stop` never returns.
#[tokio::test]
async fn pipe_stream_panic_on_a_later_write_loses_the_receiver() {
    let (stream, _peer) = tokio::io::duplex(64);
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, tx_receiver) = mpsc::channel(8);
    for _ in 0..3 {
        tx_sender
            .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
            .await
            .expect("queue tx message");
    }
    let bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer = LaterPanickingWriter {
        accept_writes: 1,
        writes: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        bytes: bytes.clone(),
    };
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    let tx_receiver = timeout(
        Duration::from_secs(1),
        super::run_pipe_stream(
            stream,
            writer,
            crate::hash::AddressHash::new_from_slice(b"pipe-tx-later-panic"),
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
    .expect("a transmit loop panic returns from the driver instead of unwinding it");

    assert!(bytes.load(Ordering::SeqCst) > 0, "the frame written before the panic reached the child");
    assert!(tx_receiver.is_none(), "the receiver died with the panicking transmit task");
    let status = runtime_status.lock().expect("pipe runtime status").clone();
    assert_eq!(status.process_state, "transmit_task_failed");
    assert!(!status.pipe_is_open);
    assert!(
        status
            .last_error
            .as_deref()
            .is_some_and(|e| e.contains("panicked") && e.contains("injected later pipe write panic")),
        "the join error names the writer panic: {:?}",
        status.last_error
    );
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
async fn pipe_stream_returns_the_receiver_after_a_receive_task_panic_with_the_writer_idle() {
    let (rx_channel, _rx_receiver) = mpsc::channel(8);
    let (tx_sender, tx_receiver) = mpsc::channel(8);
    let runtime_status = Arc::new(std::sync::Mutex::new(super::PipeRuntimeStatus::new("test")));
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));

    let mut tx_receiver = timeout(
        Duration::from_secs(1),
        super::run_pipe_stream(
            PanickingReader,
            tokio::io::sink(),
            crate::hash::AddressHash::new_from_slice(b"pipe-rx-panic"),
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
    .expect("a receive task panic stops the writer and returns from the driver")
    .expect("the receiver comes back");

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
