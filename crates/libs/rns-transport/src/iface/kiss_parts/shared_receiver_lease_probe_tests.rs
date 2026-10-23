// Test code holds guards across awaits on purpose.
#![allow(clippy::await_holding_invalid_type)]
// Spec-first probes of the shared-receiver KISS entry point after the
// per-receive restore (`ReceiverLease`):
//
// 1. "the guard spans only this recv": while a co-consumer HOLDS the tx
//    receiver lock, the stream still decodes inbound frames and still obeys
//    cancellation. A per-pass lock (the shape before the owned-receiver
//    change) parks the whole loop on the mutex, so neither happens.
// 2. baseline-equivalent co-consumer semantics: a co-consumer that takes the
//    lock after a pass consumes the next queued packet itself, the stream
//    does not transmit it, and the stream resumes transmitting once the
//    co-consumer releases. This holds for the per-pass and the per-receive
//    shapes alike; it is the public contract callers of the shared receiver
//    rely on.
// 3. the shared management receiver is leased per receive too: a frame fed
//    through it reaches the wire as a raw KISS frame, and a co-consumer
//    queued on its mutex is served between passes (a lock held for the life
//    of the stream leaves it queued).
use super::*;
use crate::hash::AddressHash;
use crate::iface::{RxMessage, TxMessage, TxMessageType};
use crate::kiss::encode_data_frame;
use crate::packet::{Packet, PacketDataBuffer};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
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

fn packet_for(iface_address: AddressHash, tag: u8, body: &[u8]) -> TxMessage {
    TxMessage {
        tx_type: TxMessageType::Direct(iface_address),
        packet: Packet {
            destination: AddressHash::new_from_slice(&[tag; 16]),
            data: PacketDataBuffer::new_from_slice(body),
            ..Packet::default()
        },
    }
}

async fn wait_until_stream_holds(lock: &Arc<tokio::sync::Mutex<mpsc::Receiver<TxMessage>>>) {
    timeout(Duration::from_secs(1), async {
        while lock.try_lock().is_ok() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the running stream should hold the tx receiver lock while it waits for a packet");
}

async fn read_some(peer_stream: &mut tokio::io::DuplexStream, what: &str) -> Vec<u8> {
    let mut wire = [0_u8; 256];
    let read = timeout(Duration::from_secs(1), peer_stream.read(&mut wire))
        .await
        .unwrap_or_else(|_| panic!("the stream should put {what} on the wire"))
        .expect("read the KISS frame");
    assert!(read > 0, "{what} must put bytes on the wire");
    wire[..read].to_vec()
}

/// Spec 1: a co-consumer holding the tx lock does not stop inbound decoding
/// or cancellation; the guard spans only the stream's own `recv`.
#[tokio::test]
async fn kiss_stream_with_ifac_keeps_reading_and_cancelling_while_a_co_consumer_holds_the_tx_lock()
{
    let iface_address = AddressHash::new_from_slice(&[0x71; 16]);
    let (rx_channel, mut rx_queue) = mpsc::channel::<RxMessage>(8);
    let (tx_queue, tx_receiver) = mpsc::channel::<TxMessage>(8);
    let tx_channel = Arc::new(tokio::sync::Mutex::new(tx_receiver));
    let (stream, mut peer_stream) = tokio::io::duplex(4_096);
    let cancel = CancellationToken::new();
    let ifac_state: IfacState = Arc::new(std::sync::RwLock::new(None));
    let session = tokio::spawn(run_kiss_stream_with_ifac(
        stream,
        probe_options(iface_address),
        cancel.clone(),
        rx_channel,
        tx_channel.clone(),
        ifac_state.clone(),
        Arc::new(std::sync::atomic::AtomicU64::new(0)),
    ));
    wait_until_stream_holds(&tx_channel).await;

    // Queue a co-consumer that will HOLD the lock until released.
    let (release_tx, release_rx) = oneshot::channel::<()>();
    let (held_tx, held_rx) = oneshot::channel::<()>();
    let holder_channel = tx_channel.clone();
    let holder = tokio::spawn(async move {
        let guard = holder_channel.lock().await;
        held_tx.send(()).expect("report the lock as held");
        let _ = release_rx.await;
        drop(guard);
    });

    // One transmit pass hands the guard to the queued co-consumer.
    tx_queue
        .send(packet_for(iface_address, 0x72, b"first pass"))
        .await
        .expect("queue a packet for the stream");
    read_some(&mut peer_stream, "the first pass").await;
    timeout(Duration::from_secs(1), held_rx)
        .await
        .expect("the co-consumer must get the lock between passes")
        .expect("holder reports");
    assert!(tx_channel.try_lock().is_err(), "the co-consumer holds the tx lock");

    // Inbound traffic must still be decoded and delivered.
    let inbound = Packet {
        destination: AddressHash::new_from_slice(&[0x73; 16]),
        data: PacketDataBuffer::new_from_slice(b"inbound while held"),
        ..Packet::default()
    };
    let payload = encode_packet_ifac(&ifac_state, &inbound).expect("encode inbound packet");
    peer_stream
        .write_all(&encode_data_frame(&payload))
        .await
        .expect("write the inbound KISS frame");
    let received = timeout(Duration::from_secs(1), rx_queue.recv())
        .await
        .expect("inbound frames must still be decoded while a co-consumer holds the tx lock")
        .expect("rx channel open");
    assert_eq!(received.address, iface_address);
    assert_eq!(received.packet.destination, inbound.destination);
    assert_eq!(received.packet.data.as_slice(), b"inbound while held");

    // Cancellation must still be observed while the lock is held elsewhere.
    cancel.cancel();
    timeout(Duration::from_secs(1), session)
        .await
        .expect("the stream must stop on cancel while a co-consumer holds the tx lock")
        .expect("join KISS stream");

    assert!(!holder.is_finished(), "the co-consumer still holds the lock after the stream exits");
    release_tx.send(()).expect("release the holder");
    timeout(Duration::from_secs(1), holder).await.expect("holder exits").expect("join holder");
}

/// Spec 2: a co-consumer served between passes consumes the next queued
/// packet itself; the stream transmits neither that packet nor anything
/// else until the co-consumer releases, then resumes.
#[tokio::test]
async fn kiss_stream_with_ifac_lets_a_co_consumer_take_a_packet_then_resumes_transmitting() {
    let iface_address = AddressHash::new_from_slice(&[0x74; 16]);
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
    wait_until_stream_holds(&tx_channel).await;

    // The co-consumer takes exactly one packet, then releases the lock.
    let consumer_channel = tx_channel.clone();
    let consumer = tokio::spawn(async move {
        let mut guard = consumer_channel.lock().await;
        guard.recv().await.expect("tx channel open")
    });
    sleep(Duration::from_millis(20)).await;
    assert!(!consumer.is_finished(), "the co-consumer queues behind the running stream");

    // Pass 1: the stream still owns the receive and transmits.
    tx_queue
        .send(packet_for(iface_address, 0x75, b"stream sends this"))
        .await
        .expect("queue packet 1");
    let wire = read_some(&mut peer_stream, "packet 1").await;
    assert!(
        wire.windows(b"stream sends this".len()).any(|w| w == b"stream sends this"),
        "packet 1 is transmitted by the stream"
    );

    // Pass boundary: the fair mutex hands the guard to the co-consumer, which
    // now owns the next receive.
    tx_queue
        .send(packet_for(iface_address, 0x76, b"co-consumer takes this"))
        .await
        .expect("queue packet 2");
    let taken = timeout(Duration::from_secs(1), consumer)
        .await
        .expect("the co-consumer must receive the packet queued after the pass")
        .expect("join co-consumer");
    assert_eq!(taken.packet.data.as_slice(), b"co-consumer takes this");
    let mut wire = [0_u8; 256];
    assert!(
        timeout(Duration::from_millis(200), peer_stream.read(&mut wire)).await.is_err(),
        "the stream must not transmit a packet the co-consumer consumed"
    );

    // The co-consumer released; the stream owns the receive again.
    tx_queue
        .send(packet_for(iface_address, 0x77, b"stream sends again"))
        .await
        .expect("queue packet 3");
    let wire = read_some(&mut peer_stream, "packet 3").await;
    assert!(
        wire.windows(b"stream sends again".len()).any(|w| w == b"stream sends again"),
        "the stream resumes transmitting after the co-consumer releases"
    );

    cancel.cancel();
    timeout(Duration::from_secs(1), session)
        .await
        .expect("KISS stream should stop after cancellation")
        .expect("join KISS stream");
}

/// Spec 3: the shared management receiver is leased per receive: frames fed
/// through it reach the wire raw, and a locker queued on its mutex is served
/// between passes.
#[tokio::test]
async fn kiss_stream_with_ifac_leases_the_shared_management_receiver_per_receive() {
    let iface_address = AddressHash::new_from_slice(&[0x78; 16]);
    let (rx_channel, _rx_queue) = mpsc::channel::<RxMessage>(8);
    let (_tx_queue, tx_receiver) = mpsc::channel::<TxMessage>(8);
    let tx_channel = Arc::new(tokio::sync::Mutex::new(tx_receiver));
    let (management_tx, management_rx) = mpsc::channel::<Vec<u8>>(8);
    let management_channel = Arc::new(tokio::sync::Mutex::new(management_rx));
    let (stream, mut peer_stream) = tokio::io::duplex(4_096);
    let cancel = CancellationToken::new();
    let mut options = probe_options(iface_address);
    options.management_frame_rx = Some(management_channel.clone());
    let session = tokio::spawn(run_kiss_stream_with_ifac(
        stream,
        options,
        cancel.clone(),
        rx_channel,
        tx_channel.clone(),
        Arc::new(std::sync::RwLock::new(None)),
        Arc::new(std::sync::atomic::AtomicU64::new(0)),
    ));
    timeout(Duration::from_secs(1), async {
        while management_channel.try_lock().is_ok() {
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the running stream should hold the management receiver lock while it waits");

    let waiter_channel = management_channel.clone();
    let waiter = tokio::spawn(async move {
        drop(waiter_channel.lock().await);
    });
    sleep(Duration::from_millis(20)).await;
    assert!(!waiter.is_finished(), "the waiter must queue behind the running stream");

    // Drive one management pass: the raw frame must appear verbatim on the wire.
    let raw_frame = encode_data_frame(b"mgmt probe");
    management_tx.send(raw_frame.clone()).await.expect("queue a management frame");
    let wire = read_some(&mut peer_stream, "the management frame").await;
    assert_eq!(wire, raw_frame, "management frames are written raw, byte for byte");

    timeout(Duration::from_secs(1), waiter)
        .await
        .expect("a locker queued on the shared management receiver must be served between passes")
        .expect("join waiter");

    cancel.cancel();
    timeout(Duration::from_secs(1), session)
        .await
        .expect("KISS stream should stop after cancellation")
        .expect("join KISS stream");
}
