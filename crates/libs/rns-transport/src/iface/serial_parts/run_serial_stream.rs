struct SerialStreamOptions {
    iface_address: AddressHash,
    device: String,
    mtu: usize,
    cancel: CancellationToken,
    rx_channel: tokio::sync::mpsc::Sender<RxMessage>,
    tx_channel: InterfaceTxReceiver,
    runtime_status: SerialRuntimeStatusHandle,
}

#[cfg(test)]
async fn run_serial_stream<IO>(
    stream: IO,
    options: SerialStreamOptions,
) where
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    run_serial_stream_with_ifac(
        stream,
        options,
        Arc::new(std::sync::RwLock::new(None)),
        Arc::new(std::sync::atomic::AtomicU64::new(0)),
    )
    .await;
}

/// Pumps one opened port and returns the tx receiver so the caller can carry
/// it, with any queued messages, into the next open attempt. The reader runs
/// as its own task; the writer loop runs here because it owns the receiver.
async fn run_serial_stream_with_ifac<IO>(
    stream: IO,
    options: SerialStreamOptions,
    ifac_state: IfacState,
    ifac_violations: Arc<std::sync::atomic::AtomicU64>,
) -> InterfaceTxReceiver
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let stop = CancellationToken::new();
    let (mut read_port, mut write_port) = tokio::io::split(stream);
    let SerialStreamOptions {
        iface_address,
        device,
        mtu,
        cancel,
        rx_channel,
        mut tx_channel,
        runtime_status,
    } = options;
    let rx_device = device.clone();
    let tx_device = device;
    runtime_status.update(|status| {
        status.link_state = "running".to_string();
        status.last_error = None;
    });

    let mut rx_task = {
        let cancel = cancel.clone();
        let stop = stop.clone();
        let rx_channel = rx_channel.clone();
        let runtime_status = runtime_status.clone();
        let ifac_state = ifac_state.clone();
        let ifac_violations = ifac_violations.clone();
        tokio::spawn(async move {
            let mut hdlc_rx_buffer = vec![0_u8; mtu.saturating_add(MAX_IFAC_SIZE_BYTES)];
            let mut frame_buffer = Vec::<u8>::with_capacity(mtu * 4);
            let mut read_buffer = vec![0_u8; mtu.max(256)];

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = stop.cancelled() => break,
                    result = read_port.read(&mut read_buffer[..]) => {
                        match result {
                            Ok(0) => {
                                log::warn!(
                                    "EOF on iface={} device={}",
                                    iface_address,
                                    rx_device
                                );
                                runtime_status.update(|status| {
                                    status.link_state = "eof".to_string();
                                    status.eof_count = status.eof_count.saturating_add(1);
                                });
                                stop.cancel();
                                break;
                            }
                            Ok(n) => {
                                runtime_status.update(|status| {
                                    status.bytes_rx = status.bytes_rx.saturating_add(n as u64);
                                });
                                frame_buffer.extend_from_slice(&read_buffer[..n]);

                                while let Some((start, end)) = Hdlc::find(&frame_buffer) {
                                    let frame = &frame_buffer[start..=end];
                                    let mut output = OutputBuffer::new(&mut hdlc_rx_buffer[..]);
                                    if Hdlc::decode(frame, &mut output).is_ok() {
                                        runtime_status.update(|status| {
                                            status.link_state = "running".to_string();
                                            status.frames_rx = status.frames_rx.saturating_add(1);
                                            status.last_error = None;
                                        });
                                        match decode_packet_ifac(&ifac_state, output.as_slice()) {
                                            Ok(packet) => {
                                            match rx_channel
                                                .send(RxMessage {
                                                    address: iface_address,
                                                    packet,
                                                    source: IfaceSource::None,
                                                })
                                                .await
                                            {
                                                Ok(()) => {
                                                    runtime_status.update(|status| {
                                                        status.packets_rx =
                                                            status.packets_rx.saturating_add(1);
                                                    });
                                                }
                                                Err(err) => {
                                                    runtime_status.update(|status| {
                                                        status.rx_queue_errors = status
                                                            .rx_queue_errors
                                                            .saturating_add(1);
                                                        status.last_error = Some(err.to_string());
                                                    });
                                                }
                                            }
                                            }
                                            Err(error) => {
                                                if is_ifac_violation(&error) {
                                                    record_ifac_violation(&ifac_violations, &error);
                                                }
                                                runtime_status.update(|status| {
                                                    status.deserialize_errors =
                                                        status.deserialize_errors.saturating_add(1);
                                                    status.last_error =
                                                        Some("packet deserialize failed".to_string());
                                                });
                                            }
                                        }
                                    } else {
                                        runtime_status.update(|status| {
                                            status.decode_errors =
                                                status.decode_errors.saturating_add(1);
                                            status.last_error = Some("hdlc decode failed".to_string());
                                        });
                                    }
                                    frame_buffer.drain(..=end);
                                }

                                if frame_buffer.len() > mtu * 64 {
                                    frame_buffer.clear();
                                    runtime_status.update(|status| {
                                        status.decode_errors =
                                            status.decode_errors.saturating_add(1);
                                        status.last_error =
                                            Some("serial frame buffer overflow".to_string());
                                    });
                                }
                            }
                            Err(err) => {
                                log::warn!(
                                    "read error iface={} device={} err={}",
                                    iface_address,
                                    rx_device,
                                    err
                                );
                                runtime_status.update(|status| {
                                    status.link_state = "read_error".to_string();
                                    status.read_errors = status.read_errors.saturating_add(1);
                                    status.last_error = Some(err.to_string());
                                });
                                stop.cancel();
                                break;
                            }
                        }
                    }
                }
            }
        })
    };

    let tx_loop = {
        let cancel = cancel.clone();
        let stop = stop.clone();
        let tx_channel = &mut tx_channel;
        let runtime_status = runtime_status.clone();
        let ifac_state = ifac_state.clone();
        async move {
            loop {
                if stop.is_cancelled() {
                    break;
                }

                let mut hdlc_tx_buffer =
                    vec![0_u8; serial_wire_buffer_capacity(mtu.saturating_add(MAX_IFAC_SIZE_BYTES))];

                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = stop.cancelled() => break,
                    Some(message) = tx_channel.recv() => {
                        match encode_packet_ifac(&ifac_state, &message.packet) {
                            Ok(payload) => {
                                let mut hdlc_output = OutputBuffer::new(&mut hdlc_tx_buffer[..]);
                                if Hdlc::encode(&payload, &mut hdlc_output).is_ok() {
                                    if let Err(err) = write_port.write_all(hdlc_output.as_slice()).await {
                                        log::warn!(
                                            "write error iface={} device={} err={}",
                                            iface_address,
                                            tx_device,
                                            err
                                        );
                                        runtime_status.update(|status| {
                                            status.link_state = "write_error".to_string();
                                            status.tx_errors = status.tx_errors.saturating_add(1);
                                            status.last_error = Some(err.to_string());
                                        });
                                        stop.cancel();
                                        break;
                                    }
                                    if let Err(err) = write_port.flush().await {
                                        log::warn!(
                                            "flush error iface={} device={} err={}",
                                            iface_address,
                                            tx_device,
                                            err
                                        );
                                        runtime_status.update(|status| {
                                            status.link_state = "flush_error".to_string();
                                            status.tx_errors = status.tx_errors.saturating_add(1);
                                            status.last_error = Some(err.to_string());
                                        });
                                        stop.cancel();
                                        break;
                                    }
                                    runtime_status.update(|status| {
                                        status.link_state = "running".to_string();
                                        status.packets_tx = status.packets_tx.saturating_add(1);
                                        status.frames_tx = status.frames_tx.saturating_add(1);
                                        status.bytes_tx = status
                                            .bytes_tx
                                            .saturating_add(hdlc_output.as_slice().len() as u64);
                                        status.last_error = None;
                                    });
                                } else {
                                    log::warn!(
                                        "hdlc encode failed iface={} device={} payload_len={}",
                                        iface_address,
                                        tx_device,
                                        payload.len()
                                    );
                                    runtime_status.update(|status| {
                                        status.hdlc_encode_errors =
                                            status.hdlc_encode_errors.saturating_add(1);
                                        status.last_error = Some("hdlc encode failed".to_string());
                                    });
                                }
                            }
                            Err(error) => {
                                log::warn!(
                                    "packet encode failed iface={} device={} mtu={} error={}",
                                    iface_address,
                                    tx_device,
                                    mtu,
                                    error
                                );
                                runtime_status.update(|status| {
                                    status.serialize_errors =
                                        status.serialize_errors.saturating_add(1);
                                    status.last_error = Some("packet serialize failed".to_string());
                                });
                            }
                        }
                    }
                }
            }
        }
    };

    // A panicking reader never reaches `stop.cancel()` and an idle writer only
    // exits on `stop`, so finish whichever half ends first, cancel `stop` and
    // collect the other. The caught writer panic keeps the receiver for the
    // reopen; the block ends the writer's borrow of it before it is returned.
    let (tx_result, rx_result) = {
        let mut tx = CatchUnwind::new(tx_loop);
        tokio::select! {
            result = &mut tx => { stop.cancel(); (result, rx_task.await) }
            joined = &mut rx_task => { stop.cancel(); (tx.await, joined) }
        }
    };
    if let Err(error) = tx_result {
        log::error!("serial transmit task failed iface={iface_address}: {error}");
        runtime_status.update(|status| {
            status.tx_errors = status.tx_errors.saturating_add(1);
            status.last_error = Some(format!("transmit task failed: {error}"));
        });
    }
    if let Err(error) = rx_result {
        log::error!("serial receive task failed iface={iface_address}: {error}");
        runtime_status.update(|status| {
            status.last_error = Some(format!("receive task failed: {error}"));
        });
    }
    runtime_status.update(|status| {
        status.link_state = "closed".to_string();
    });
    tx_channel
}

#[cfg(test)]
mod tests {
    use super::{
        bounded_backoff_next, run_serial_stream, run_serial_stream_with_ifac,
        serial_wire_buffer_capacity, SerialInterface, SerialStreamOptions,
    };
    use crate::buffer::OutputBuffer;
    use crate::hash::AddressHash;
    use crate::iface::{hdlc::Hdlc, InterfaceChannel, InterfaceContext, TxMessage, TxMessageType};
    use crate::packet::Packet;
    use crate::serde::Serialize;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;
    use tokio::time::timeout;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn wire_capacity_handles_worst_case_hdlc_escape_expansion() {
        let mtu = 512;
        let raw = vec![0x7e_u8; mtu];
        let mut wire = vec![0_u8; serial_wire_buffer_capacity(mtu)];
        let mut output = OutputBuffer::new(&mut wire[..]);

        let encoded_len = Hdlc::encode(&raw, &mut output).expect("encode worst-case payload");
        assert!(encoded_len >= (mtu * 2) + 2, "wire len must cover escaped payload plus flags");
    }

    #[test]
    fn wire_capacity_grows_with_configured_mtu() {
        assert!(serial_wire_buffer_capacity(256) < serial_wire_buffer_capacity(2048));
    }

    #[test]
    fn reconnect_backoff_growth_is_bounded() {
        assert_eq!(
            bounded_backoff_next(Duration::from_millis(500), Duration::from_millis(5_000)),
            Duration::from_millis(1_000)
        );
        assert_eq!(
            bounded_backoff_next(Duration::from_millis(4_000), Duration::from_millis(5_000)),
            Duration::from_millis(5_000)
        );
        assert_eq!(
            bounded_backoff_next(Duration::from_millis(5_000), Duration::from_millis(5_000)),
            Duration::from_millis(5_000)
        );
    }

    #[test]
    fn serial_option_helpers_reject_invalid_values() {
        let err = SerialInterface::new("dummy", 115200)
            .with_data_bits_raw(9)
            .err()
            .expect("invalid data bits");
        assert!(err.contains("serial.data_bits"));

        let err = SerialInterface::new("dummy", 115200)
            .with_stop_bits_raw(3)
            .err()
            .expect("invalid stop bits");
        assert!(err.contains("serial.stop_bits"));

        let err = SerialInterface::new("dummy", 115200)
            .with_parity_name("mark")
            .err()
            .expect("invalid parity");
        assert!(err.contains("serial.parity"));

        let err = SerialInterface::new("dummy", 115200)
            .with_flow_control_name("xonxoff")
            .err()
            .expect("invalid flow control");
        assert!(err.contains("serial.flow_control"));
    }

    #[test]
    fn preflight_open_reports_device_open_failures() {
        let err = SerialInterface::new("__definitely_not_a_device__", 115200)
            .preflight_open()
            .expect_err("invalid device should fail preflight");
        assert!(err.contains("serial preflight open failed"));
    }

    #[tokio::test]
    async fn spawn_retry_loop_honors_cancel_after_open_failures() {
        let (rx_send, _rx_recv) = InterfaceChannel::make_rx_channel(1);
        let (_tx_send, tx_recv) = InterfaceChannel::make_tx_channel(1);
        let stop = CancellationToken::new();
        let channel = InterfaceChannel::new(
            rx_send,
            tx_recv,
            AddressHash::new_from_slice(b"serial-cancel"),
            stop.clone(),
        );
        let cancel = CancellationToken::new();
        let context = InterfaceContext::<SerialInterface> {
            inner: Arc::new(Mutex::new(
                SerialInterface::new("__definitely_not_a_device__", 115200)
                    .with_reconnect_backoff(Duration::from_millis(25)),
            )),
            channel,
            cancel: cancel.clone(),
        };

        let task = tokio::spawn(async move {
            SerialInterface::spawn(context).await;
        });

        tokio::time::sleep(Duration::from_millis(90)).await;
        cancel.cancel();

        timeout(Duration::from_secs(2), task)
            .await
            .expect("serial spawn should stop after cancel")
            .expect("join serial task");
        assert!(stop.is_cancelled(), "stop token should be cancelled on shutdown");
    }

    #[tokio::test]
    async fn serial_stream_stops_after_write_failure() {
        let (io_a, io_b) = tokio::io::duplex(64);
        drop(io_b);

        let (rx_send, _rx_recv) = mpsc::channel(4);
        let (tx_send, tx_recv) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let status = SerialInterface::new("duplex", 115200).runtime_status_handle();

        let session = tokio::spawn(run_serial_stream(
            io_a,
            SerialStreamOptions {
                iface_address: AddressHash::new_from_slice(b"serial-write-fail"),
                device: "duplex".to_string(),
                mtu: 512,
                cancel: cancel.clone(),
                rx_channel: rx_send,
                tx_channel: tx_recv,
                runtime_status: status.clone(),
            },
        ));

        tx_send
            .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
            .await
            .expect("queue tx message");

        timeout(Duration::from_secs(1), session)
            .await
            .expect("session should stop on write failure")
            .expect("join session task");
        let status = status.snapshot();
        assert_eq!(status.link_state, "closed");
    }

    #[tokio::test]
    async fn serial_stream_records_successful_tx_counters() {
        let (io_a, mut io_b) = tokio::io::duplex(1024);
        let (rx_send, _rx_recv) = mpsc::channel(4);
        let (tx_send, tx_recv) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let status = SerialInterface::new("duplex", 115200).runtime_status_handle();

        let session = tokio::spawn(run_serial_stream(
            io_a,
            SerialStreamOptions {
                iface_address: AddressHash::new_from_slice(b"serial-tx-ok"),
                device: "duplex".to_string(),
                mtu: 512,
                cancel: cancel.clone(),
                rx_channel: rx_send,
                tx_channel: tx_recv,
                runtime_status: status.clone(),
            },
        ));

        tx_send
            .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
            .await
            .expect("queue tx message");
        let mut wire = vec![0_u8; 1024];
        let n = timeout(Duration::from_secs(1), io_b.read(&mut wire))
            .await
            .expect("wire read timeout")
            .expect("wire read");
        assert!(n > 0, "tx should write HDLC bytes");

        cancel.cancel();
        timeout(Duration::from_secs(1), session)
            .await
            .expect("session should stop after cancel")
            .expect("join session task");
        let status = status.snapshot();
        assert_eq!(status.packets_tx, 1);
        assert_eq!(status.frames_tx, 1);
        assert_eq!(status.bytes_tx, n as u64);
    }

    #[tokio::test]
    async fn serial_stream_records_successful_rx_counters() {
        let (io_a, mut io_b) = tokio::io::duplex(1024);
        let (rx_send, mut rx_recv) = mpsc::channel(4);
        let (_tx_send, tx_recv) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let status = SerialInterface::new("duplex", 115200).runtime_status_handle();

        let session = tokio::spawn(run_serial_stream(
            io_a,
            SerialStreamOptions {
                iface_address: AddressHash::new_from_slice(b"serial-rx-ok"),
                device: "duplex".to_string(),
                mtu: 512,
                cancel: cancel.clone(),
                rx_channel: rx_send,
                tx_channel: tx_recv,
                runtime_status: status.clone(),
            },
        ));

        let mut packet_payload = vec![0_u8; 512];
        let mut packet_output = OutputBuffer::new(&mut packet_payload[..]);
        Packet {
            data: crate::packet::PacketDataBuffer::new_from_slice(b"serial-test"),
            ..Packet::default()
        }
        .serialize(&mut packet_output)
        .expect("serialize packet");
        let mut wire = vec![0_u8; 1024];
        let mut wire_output = OutputBuffer::new(&mut wire[..]);
        let wire_len =
            Hdlc::encode(packet_output.as_slice(), &mut wire_output).expect("encode hdlc");
        io_b.write_all(&wire[..wire_len]).await.expect("write hdlc frame");

        let message = timeout(Duration::from_secs(1), rx_recv.recv())
            .await
            .expect("rx timeout")
            .expect("rx message");
        assert_eq!(message.address, AddressHash::new_from_slice(b"serial-rx-ok"));

        cancel.cancel();
        timeout(Duration::from_secs(1), session)
            .await
            .expect("session should stop after cancel")
            .expect("join session task");
        let status = status.snapshot();
        assert_eq!(status.packets_rx, 1);
        assert_eq!(status.frames_rx, 1);
        assert_eq!(status.bytes_rx, wire_len as u64);
    }

    #[tokio::test]
    async fn serial_stream_survives_malformed_frame_then_eof() {
        let (io_a, mut io_b) = tokio::io::duplex(256);
        let (rx_send, mut rx_recv) = mpsc::channel(4);
        let (_tx_send, tx_recv) = mpsc::channel(4);
        let cancel = CancellationToken::new();
        let status = SerialInterface::new("duplex", 115200).runtime_status_handle();

        let session = tokio::spawn(run_serial_stream(
            io_a,
            SerialStreamOptions {
                iface_address: AddressHash::new_from_slice(b"serial-malformed"),
                device: "duplex".to_string(),
                mtu: 512,
                cancel: cancel.clone(),
                rx_channel: rx_send,
                tx_channel: tx_recv,
                runtime_status: status.clone(),
            },
        ));

        io_b.write_all(&[0x7e, 0x7d, 0x00, 0x7e]).await.expect("write malformed frame");
        drop(io_b);

        timeout(Duration::from_secs(1), session)
            .await
            .expect("session should stop on EOF")
            .expect("join session task");
        assert!(rx_recv.try_recv().is_err(), "malformed frame must not emit packets");
        let status = status.snapshot();
        assert_eq!(status.link_state, "closed");
        assert_eq!(status.packets_rx, 0);
    }

    #[tokio::test]
    async fn serial_stream_returns_receiver_with_queued_message() {
        let (io_a, io_b) = tokio::io::duplex(64);
        drop(io_b);

        let (rx_send, _rx_recv) = mpsc::channel(4);
        let (tx_send, tx_recv) = mpsc::channel(4);
        for _ in 0..2 {
            tx_send
                .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
                .await
                .expect("queue tx message");
        }
        let status = SerialInterface::new("duplex", 115200).runtime_status_handle();

        let mut tx_recv = timeout(
            Duration::from_secs(1),
            run_serial_stream_with_ifac(
                io_a,
                SerialStreamOptions {
                    iface_address: AddressHash::new_from_slice(b"serial-keep-queue"),
                    device: "duplex".to_string(),
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
        .expect("session should stop on write failure");

        assert!(
            tx_recv.try_recv().is_ok(),
            "the message queued behind the failed write stays in the returned receiver"
        );
        assert_eq!(status.snapshot().link_state, "closed");
    }

    /// A port whose reads stay pending and whose first write panics.
    struct PanickingPort;

    impl tokio::io::AsyncRead for PanickingPort {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    impl tokio::io::AsyncWrite for PanickingPort {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            panic!("injected serial write panic")
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

    #[tokio::test]
    async fn serial_stream_returns_receiver_after_transmit_loop_panic() {
        let (rx_send, _rx_recv) = mpsc::channel(4);
        let (tx_send, tx_recv) = mpsc::channel(4);
        for _ in 0..2 {
            tx_send
                .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
                .await
                .expect("queue tx message");
        }
        let status = SerialInterface::new("panic", 115200).runtime_status_handle();

        let mut tx_recv = timeout(
            Duration::from_secs(1),
            run_serial_stream_with_ifac(
                PanickingPort,
                SerialStreamOptions {
                    iface_address: AddressHash::new_from_slice(b"serial-tx-panic"),
                    device: "panic".to_string(),
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
        .expect("a transmit loop panic returns from the driver instead of unwinding it");

        assert!(tx_recv.try_recv().is_ok(), "the receiver survives the panic with its queue");
        let status = status.snapshot();
        assert_eq!(status.link_state, "closed");
        assert_eq!(status.tx_errors, 1, "the panic counts as a transmit error like a failed write");
        // The write half panicked inside `tokio::io::split`, whose shared lock
        // is poisoned from then on, so the read task fails right after it and
        // records its own failure last.
        let last_error = status.last_error.as_deref().unwrap_or_default();
        assert!(
            last_error == "transmit task failed: injected serial write panic"
                || last_error.starts_with("receive task failed: "),
            "a task failure is recorded after the transmit loop panic: {last_error}"
        );
    }

    /// A port whose first read panics and whose writes succeed.
    struct PanickingReadPort;

    impl tokio::io::AsyncRead for PanickingReadPort {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            _: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            panic!("injected serial read panic")
        }
    }

    impl tokio::io::AsyncWrite for PanickingReadPort {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
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

    // The writer has nothing queued and no cancel, so it only exits on `stop`;
    // a pump that awaits the writer before joining the reader never returns.
    #[tokio::test]
    async fn serial_stream_returns_receiver_after_a_receive_task_panic_with_the_writer_idle() {
        let (rx_send, _rx_recv) = mpsc::channel(4);
        let (tx_send, tx_recv) = mpsc::channel(4);
        let status = SerialInterface::new("read-panic", 115200).runtime_status_handle();

        let mut tx_recv = timeout(
            Duration::from_secs(1),
            run_serial_stream_with_ifac(
                PanickingReadPort,
                SerialStreamOptions {
                    iface_address: AddressHash::new_from_slice(b"serial-rx-panic"),
                    device: "read-panic".to_string(),
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
        .expect("a receive task panic stops the writer and returns from the driver");

        let status = status.snapshot();
        assert_eq!(status.link_state, "closed");
        let last_error = status.last_error.as_deref().unwrap_or_default();
        assert!(
            last_error.starts_with("receive task failed: ")
                && last_error.contains("injected serial read panic"),
            "the join error carries the reader's panic message: {last_error}"
        );
        tx_send
            .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: Packet::default() })
            .await
            .expect("the tx sender is still live after the pump returns");
        assert!(tx_recv.try_recv().is_ok(), "the returned receiver is intact for the reopen");
    }
}
