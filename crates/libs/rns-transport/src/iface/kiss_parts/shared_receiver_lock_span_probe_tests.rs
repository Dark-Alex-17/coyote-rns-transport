// Pins the lock span of the shared-receiver KISS entry point: the stream
// takes the receiver guard per receive, not for its whole life. The sibling
// pin in `ifac_runtime_tests.rs` observes the lock while the stream idles,
// which both shapes satisfy; this one crosses a transmit pass with a
// competing locker queued, which discriminates: tokio's Mutex is fair, so a
// per-receive lock hands the guard to the waiter at the pass boundary, while
// a guard held for the life of the stream would leave it queued.
use super::*;
use crate::hash::AddressHash;
use crate::iface::{TxMessage, TxMessageType};
use crate::packet::{Packet, PacketDataBuffer};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

fn probe_options(iface_address: AddressHash) -> KissStreamOptions {
    KissStreamOptions {
        iface_address,
        device: "in-memory KISS TCP".to_string(),
        mtu: 564,
        flow_control: false,
        flow_control_timeout: KISS_FLOW_CONTROL_TIMEOUT,
        read_frame_timeout: KISS_READ_FRAME_TIMEOUT,
        initial_frames: Vec::new(),
        shutdown_frames: Vec::new(),
        id_beacon: None,
        activity_probe: None,
        payload_adapter: KissPayloadAdapter::Raw,
        strip_command_port_nibble: false,
        command_tx: None,
        data_rx_tx: None,
        management_frame_rx: None,
        runtime_status: None,
    }
}

/// A locker queued behind the running stream is served at the end of the
/// next transmit pass while the stream keeps running.
#[tokio::test]
async fn kiss_stream_with_ifac_serves_a_queued_locker_between_transmit_passes() {
    let iface_address = AddressHash::new_from_slice(&[0x66; 16]);
    let (rx_channel, _rx_queue) = mpsc::channel::<RxMessage>(8);
    let (tx_queue, tx_receiver) = mpsc::channel::<TxMessage>(8);
    let tx_channel = Arc::new(tokio::sync::Mutex::new(tx_receiver));
    let (stream, mut peer_stream) = tokio::io::duplex(4_096);
    let cancel = CancellationToken::new();
    let session = tokio::spawn(run_kiss_stream_with_ifac(
        stream,
        probe_options(iface_address),
        cancel.clone(),
        rx_channel,
        tx_channel.clone(),
        Arc::new(std::sync::RwLock::new(None)),
        Arc::new(std::sync::atomic::AtomicU64::new(0)),
    ));

    timeout(Duration::from_secs(1), async {
        while tx_channel.try_lock().is_ok() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the running stream should hold the tx receiver lock");

    // Queue a competing locker. tokio's Mutex is fair: whoever releases next
    // hands the guard to this waiter.
    let waiter_channel = tx_channel.clone();
    let waiter = tokio::spawn(async move {
        drop(waiter_channel.lock().await);
    });
    sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished(), "the waiter must queue behind the running stream");

    // Drive one full transmit pass and prove it happened on the wire.
    tx_queue
        .send(TxMessage {
            tx_type: TxMessageType::Direct(iface_address),
            packet: Packet {
                destination: AddressHash::new_from_slice(&[0x67; 16]),
                data: PacketDataBuffer::new_from_slice(b"one transmit pass"),
                ..Packet::default()
            },
        })
        .await
        .expect("queue a packet for the stream");
    let mut wire = [0_u8; 64];
    let read = timeout(Duration::from_secs(1), peer_stream.read(&mut wire))
        .await
        .expect("the stream should transmit the queued packet")
        .expect("read the KISS frame");
    assert!(read > 0, "the transmit pass must put bytes on the wire");

    timeout(Duration::from_secs(1), waiter)
        .await
        .expect("the queued locker must be served between transmit passes; it stayed queued behind the running stream")
        .expect("join waiter");

    cancel.cancel();
    timeout(Duration::from_secs(1), session)
        .await
        .expect("KISS stream should stop after cancellation")
        .expect("join KISS stream");
}
