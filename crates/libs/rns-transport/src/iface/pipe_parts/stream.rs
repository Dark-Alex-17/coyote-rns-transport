use alloc::vec::Vec;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use super::status::{mark_task_failed, update_pipe_status};
use super::{InterfaceTxReceiver, PipeRuntimeStatus};
use crate::buffer::OutputBuffer;
use crate::hash::AddressHash;
use crate::iface::{
    decode_packet_ifac, encode_packet_ifac, hdlc::Hdlc, is_ifac_violation, record_ifac_violation,
    IfacState, IfaceSource, RxMessage, MAX_IFAC_SIZE_BYTES,
};

/// Pumps one spawned process. The reader and the writer each run as their own
/// task; `tx_channel` travels into the writer and comes back through its join
/// handle on a normal exit so the caller can carry it into the respawn.
/// Returns once both sides have exited; `None` means the writer panicked and
/// took the receiver with it.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run_pipe_stream<R, W>(
    mut reader: R,
    mut writer: W,
    iface_address: AddressHash,
    mtu: usize,
    cancel: CancellationToken,
    iface_stop: CancellationToken,
    rx_channel: tokio::sync::mpsc::Sender<RxMessage>,
    mut tx_channel: InterfaceTxReceiver,
    runtime_status: Arc<std::sync::Mutex<PipeRuntimeStatus>>,
    ifac_state: IfacState,
    ifac_violations: Arc<std::sync::atomic::AtomicU64>,
) -> Option<InterfaceTxReceiver>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let stop = CancellationToken::new();
    let rx_stop = stop.clone();
    let tx_stop = stop.clone();

    let mut rx_task = {
        let cancel = cancel.clone();
        let iface_stop = iface_stop.clone();
        let runtime_status = runtime_status.clone();
        let ifac_state = ifac_state.clone();
        let ifac_violations = ifac_violations.clone();
        tokio::spawn(async move {
            let mut hdlc_rx_buffer = vec![0_u8; mtu.saturating_add(MAX_IFAC_SIZE_BYTES)];
            let mut frame_buffer = Vec::<u8>::with_capacity(mtu * 4);
            let mut read_buffer = vec![0_u8; mtu.clamp(256, 32_768)];

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = iface_stop.cancelled() => break,
                    _ = rx_stop.cancelled() => break,
                    result = reader.read(&mut read_buffer[..]) => {
                        match result {
                            Ok(0) => {
                                update_pipe_status(&runtime_status, |status| {
                                    status.process_state = "closed".to_string();
                                    status.pipe_is_open = false;
                                    status.last_error = Some("pipe stdout closed".to_string());
                                });
                                rx_stop.cancel();
                                break;
                            }
                            Ok(n) => {
                                frame_buffer.extend_from_slice(&read_buffer[..n]);
                                while let Some((start, end)) = Hdlc::find(&frame_buffer) {
                                    let frame = &frame_buffer[start..=end];
                                    let mut output = OutputBuffer::new(&mut hdlc_rx_buffer[..]);
                                    if Hdlc::decode(frame, &mut output).is_ok() {
                                        match decode_packet_ifac(&ifac_state, output.as_slice()) {
                                            Ok(packet) => {
                                            if rx_channel
                                                .send(RxMessage {
                                                    address: iface_address,
                                                    packet,
                                                    source: IfaceSource::None,
                                                })
                                                .await
                                                .is_err()
                                            {
                                                log::warn!(
                                                    "pipe receive queue closed iface={iface_address}"
                                                );
                                                update_pipe_status(&runtime_status, |status| {
                                                    status.process_state = "closed".to_string();
                                                    status.pipe_is_open = false;
                                                    status.last_error = Some(
                                                        "transport receive queue closed".to_string(),
                                                    );
                                                });
                                                rx_stop.cancel();
                                                return;
                                            }
                                            }
                                            Err(error) => {
                                                if is_ifac_violation(&error) {
                                                    record_ifac_violation(&ifac_violations, &error);
                                                }
                                                log::debug!("pipe packet rejected: {error}");
                                            }
                                        }
                                    }
                                    frame_buffer.drain(..=end);
                                }

                                if frame_buffer.len() > mtu * 64 {
                                    frame_buffer.clear();
                                }
                            }
                            Err(err) => {
                                log::warn!("pipe read error iface={} err={}", iface_address, err);
                                update_pipe_status(&runtime_status, |status| {
                                    status.process_state = "read_error".to_string();
                                    status.pipe_is_open = false;
                                    status.last_error = Some(err.to_string());
                                });
                                rx_stop.cancel();
                                break;
                            }
                        }
                    }
                }
            }
        })
    };

    let mut tx_task = {
        let cancel = cancel.clone();
        let iface_stop = iface_stop.clone();
        let runtime_status = runtime_status.clone();
        let ifac_state = ifac_state.clone();
        tokio::spawn(async move {
            loop {
                if tx_stop.is_cancelled() {
                    break;
                }

                let mut hdlc_tx_buffer = vec![
                    0_u8;
                    mtu.saturating_add(MAX_IFAC_SIZE_BYTES)
                        .saturating_mul(2)
                        .saturating_add(16)
                ];

                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = iface_stop.cancelled() => break,
                    _ = tx_stop.cancelled() => break,
                    Some(message) = tx_channel.recv() => {
                        let payload = match encode_packet_ifac(&ifac_state, &message.packet) {
                            Ok(payload) => payload,
                            Err(err) => {
                                log::warn!(
                                    "pipe packet encode failed iface={} err={:?}",
                                    iface_address,
                                    err
                                );
                                continue;
                            }
                        };
                        let mut hdlc_output = OutputBuffer::new(&mut hdlc_tx_buffer[..]);
                        if Hdlc::encode(&payload, &mut hdlc_output).is_ok() {
                            if let Err(err) = writer.write_all(hdlc_output.as_slice()).await {
                                log::warn!("pipe write error iface={} err={}", iface_address, err);
                                update_pipe_status(&runtime_status, |status| {
                                    status.process_state = "write_error".to_string();
                                    status.pipe_is_open = false;
                                    status.last_error = Some(err.to_string());
                                });
                                tx_stop.cancel();
                                break;
                            }
                            if let Err(err) = writer.flush().await {
                                log::warn!("pipe flush error iface={} err={}", iface_address, err);
                                update_pipe_status(&runtime_status, |status| {
                                    status.process_state = "write_error".to_string();
                                    status.pipe_is_open = false;
                                    status.last_error = Some(err.to_string());
                                });
                                tx_stop.cancel();
                                break;
                            }
                        }
                    }
                }
            }
            tx_channel
        })
    };

    // A panicking reader never reaches `rx_stop.cancel()` and an idle writer only
    // exits on `stop`, so finish whichever half ends first, cancel `stop` and
    // collect the other.
    let (tx_result, rx_result) = tokio::select! {
        joined = &mut tx_task => { stop.cancel(); (joined, rx_task.await) }
        joined = &mut rx_task => { stop.cancel(); (tx_task.await, joined) }
    };
    let tx_channel = match tx_result {
        Ok(tx_channel) => Some(tx_channel),
        Err(error) => {
            log::error!("pipe transmit task failed iface={iface_address}: {error}");
            mark_task_failed(&runtime_status, "transmit_task_failed", error.to_string());
            None
        }
    };
    if let Err(error) = rx_result {
        log::error!("pipe receive task failed iface={iface_address}: {error}");
        mark_task_failed(&runtime_status, "receive_task_failed", error.to_string());
    }
    tx_channel
}
