use super::*;
use crate::iface::{InterfaceChannel, InterfaceContext};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn tcp_client_delivers_tx_message_queued_between_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(1);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(Duration::from_millis(50))
        .with_disconnect_events(disconnect_tx);
    let (rx_send, _rx_recv) = InterfaceChannel::make_rx_channel(8);
    let (tx_send, tx_recv) = InterfaceChannel::make_tx_channel(8);
    let iface_address = AddressHash::new_from_slice(b"tcp-tx-queue-reconnect");
    let channel = InterfaceChannel::new(rx_send, tx_recv, iface_address, CancellationToken::new());
    let cancel = CancellationToken::new();
    let context = InterfaceContext::<TcpClient> {
        inner: Arc::new(Mutex::new(client)),
        channel,
        cancel: cancel.clone(),
    };
    let task = tokio::spawn(TcpClient::spawn(context));

    let (first_stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
        .await
        .expect("initial connection deadline")
        .expect("accept initial connection");
    // Closing the listener before the stream leaves the client nothing to
    // redial until the port is bound again below.
    drop(listener);
    drop(first_stream);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(2), disconnect_rx.recv())
            .await
            .expect("disconnect event deadline")
            .expect("disconnect event"),
        iface_address
    );

    let packet = Packet {
        data: PacketDataBuffer::new_from_slice(b"queued-while-disconnected"),
        ..Packet::default()
    };
    let mut raw = vec![0_u8; TcpClient::DEFAULT_MTU];
    let raw_len = {
        let mut output = OutputBuffer::new(&mut raw);
        packet.serialize(&mut output).expect("serialize packet");
        output.offset()
    };
    tx_send
        .send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet })
        .await
        .expect("queue tx message while disconnected");

    let listener = TcpListener::bind(addr).await.expect("rebind listener");
    let (mut reconnected_stream, _) =
        tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .expect("reconnect deadline")
            .expect("accept reconnected stream");

    let mut wire = Vec::new();
    let mut decoded = vec![0_u8; TcpClient::DEFAULT_MTU];
    let delivered = tokio::time::timeout(Duration::from_secs(2), async {
        let mut chunk = vec![0_u8; 4096];
        loop {
            let n = reconnected_stream.read(&mut chunk).await.expect("read reconnected stream");
            assert!(n > 0, "reconnected stream closed before the queued frame arrived");
            wire.extend_from_slice(&chunk[..n]);
            while let Some((start, end)) = Hdlc::find(&wire) {
                let mut output = OutputBuffer::new(&mut decoded);
                if Hdlc::decode(&wire[start..=end], &mut output).is_ok()
                    && output.as_slice() == &raw[..raw_len]
                {
                    return;
                }
                wire.drain(..=end);
            }
        }
    })
    .await;
    assert!(delivered.is_ok(), "queued packet must be written on the reconnected stream");

    cancel.cancel();
    drop(reconnected_stream);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("TCP client task shutdown deadline")
        .expect("TCP client task");
}
