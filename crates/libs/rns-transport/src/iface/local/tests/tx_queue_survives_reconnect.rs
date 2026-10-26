use super::*;
use crate::buffer::OutputBuffer;
use crate::iface::{TxMessage, TxMessageType};
use crate::packet::PacketDataBuffer;

#[tokio::test]
async fn local_unix_client_delivers_tx_message_queued_between_connections() {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().join("reticulum-local-queue.sock");
    let endpoint = LocalUnixEndpoint::filesystem(path.clone());
    let mut manager = InterfaceManager::new(8);
    let (disconnect_tx, mut disconnect_rx) = tokio::sync::mpsc::channel(32);
    let context = manager.new_context(
        LocalUnixClient::new_connect(endpoint.clone())
            .with_reconnect_wait(Duration::from_millis(20))
            .with_disconnect_events(disconnect_tx),
    );
    let iface_address = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    let first_listener = UnixListener::bind(endpoint.tokio_path()).expect("bind first socket");
    let (first_stream, _) = tokio::time::timeout(Duration::from_secs(2), first_listener.accept())
        .await
        .expect("initial connection deadline")
        .expect("accept initial connection");
    // Tear the listener down before the stream so the client has nothing to
    // redial until the socket is bound again below.
    drop(first_listener);
    remove_socket_file(&path).expect("remove first socket");
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
    let raw = packet.to_bytes().expect("serialize packet");
    let trace =
        manager.send(TxMessage { tx_type: TxMessageType::Direct(iface_address), packet }).await;
    assert_eq!(
        trace.sent_ifaces, 1,
        "the manager queues a packet for a client that is between connections"
    );

    let second_listener = UnixListener::bind(endpoint.tokio_path()).expect("bind second socket");
    let (mut reconnected_stream, _) =
        tokio::time::timeout(Duration::from_secs(5), second_listener.accept())
            .await
            .expect("reconnect deadline")
            .expect("accept reconnected stream");

    let mut wire = Vec::new();
    let delivered = tokio::time::timeout(Duration::from_secs(2), async {
        let mut chunk = vec![0_u8; 4096];
        loop {
            let n = reconnected_stream.read(&mut chunk).await.expect("read reconnected stream");
            assert!(n > 0, "reconnected stream closed before the queued frame arrived");
            wire.extend_from_slice(&chunk[..n]);
            while let Some((start, end)) = Hdlc::find(&wire) {
                let mut decoded = vec![0_u8; end - start];
                let decoded_len =
                    Hdlc::decode(&wire[start..=end], &mut OutputBuffer::new(&mut decoded))
                        .expect("decode reconnected frame");
                if decoded[..decoded_len] == raw[..] {
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
    drop(second_listener);
    let _ = remove_socket_file(&path);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("local unix client task shutdown deadline")
        .expect("local unix client task");
}
