use super::*;
use crate::iface::{InterfaceChannel, TxMessageType};
use crate::packet::{Packet, PacketDataBuffer};
use tokio::net::TcpListener;
use tokio::time::{sleep, timeout};

#[tokio::test]
async fn kiss_tcp_client_delivers_tx_message_queued_between_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let interface = KissTcpClientInterface::new(addr.to_string())
        .with_reconnect_backoff(Duration::from_millis(50));
    let runtime_status = interface.runtime_status_handle();
    let (rx_send, _rx_recv) = InterfaceChannel::make_rx_channel(8);
    let (tx_send, tx_recv) = InterfaceChannel::make_tx_channel(8);
    let iface_address = AddressHash::new_from_slice(b"kiss-tx-queue-reconnect");
    let channel = InterfaceChannel::new(rx_send, tx_recv, iface_address, CancellationToken::new());
    let ifac_state = channel.ifac_state.clone();
    let cancel = CancellationToken::new();
    let context = InterfaceContext::<KissTcpClientInterface> {
        inner: Arc::new(Mutex::new(interface)),
        channel,
        cancel: cancel.clone(),
    };
    let task = tokio::spawn(KissTcpClientInterface::spawn(context));

    let (first_stream, _) = timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("initial connection deadline")
        .expect("accept initial connection");
    // Closing the listener before the stream leaves the client nothing to
    // redial until the port is bound again below.
    drop(listener);
    drop(first_stream);
    timeout(Duration::from_secs(2), async {
        loop {
            let link_state = runtime_status.snapshot().link_state;
            if link_state == "closed" || link_state == "connect_failed" {
                break;
            }
            sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("first stream should close after the peer hangs up");

    let packet = Packet {
        destination: AddressHash::new_from_slice(&[0x64; 16]),
        data: PacketDataBuffer::new_from_slice(b"queued-while-disconnected"),
        ..Packet::default()
    };
    tx_send
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet: packet.clone() })
        .await
        .expect("queue tx message while disconnected");

    let listener = TcpListener::bind(addr).await.expect("rebind listener");
    let (mut reconnected_stream, _) = timeout(Duration::from_secs(5), listener.accept())
        .await
        .expect("reconnect deadline")
        .expect("accept reconnected stream");

    let mut decoder = KissStreamDecoder::new(564).with_command_port_nibble_stripping(false);
    let delivered = timeout(Duration::from_secs(2), async {
        let mut chunk = vec![0_u8; 1024];
        loop {
            let n = reconnected_stream.read(&mut chunk).await.expect("read reconnected stream");
            assert!(n > 0, "reconnected stream closed before the queued frame arrived");
            for frame in decoder.push_bytes(&chunk[..n]).expect("decode KISS frames") {
                if let KissFrame::Data(payload) = frame {
                    let decoded =
                        decode_packet_ifac(&ifac_state, &payload).expect("decode queued packet");
                    if decoded.data.as_slice() == packet.data.as_slice() {
                        return;
                    }
                }
            }
        }
    })
    .await;
    assert!(delivered.is_ok(), "queued packet must be written on the reconnected stream");

    cancel.cancel();
    drop(reconnected_stream);
    timeout(Duration::from_secs(2), task)
        .await
        .expect("KISS client task shutdown deadline")
        .expect("KISS client task");
}
