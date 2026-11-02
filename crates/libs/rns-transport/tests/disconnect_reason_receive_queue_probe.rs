//! The one arm of `with_disconnect_reason_events` the loopback pins leave
//! out: a stream the client ended itself because the transport's receive
//! queue closed. Both `TcpClient` and `LocalUnixClient` document that such
//! a stream sends nothing on the reason channel, while the plain
//! `with_disconnect_events` channel, whose contract is every established
//! stream that ends while the client is neither cancelled nor stopped,
//! still fires and the client dials again. Each test first reads a real
//! `PeerClosed` from a peer that drops its end, so the reason channel is
//! known to be wired, then drops the `InterfaceManager` while the second
//! stream is up and has the peer write one framed packet, which the client
//! decodes and cannot hand to the transport. The third accept proves the
//! stream ended and the client is still dialing; the reason channel has
//! nothing queued and closes empty once the client is cancelled.

use std::time::Duration;

use rns_transport::buffer::OutputBuffer;
use rns_transport::hash::AddressHash;
use rns_transport::iface::hdlc::Hdlc;
#[cfg(unix)]
use rns_transport::iface::local::{LocalUnixClient, LocalUnixEndpoint};
use rns_transport::iface::tcp_client::TcpClient;
use rns_transport::iface::InterfaceManager;
use rns_transport::packet::{Packet, PacketDataBuffer};
use rns_transport::serde::Serialize;
use rns_transport::transport::DisconnectReason;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::time::timeout;

const RECONNECT_WAIT: Duration = Duration::from_millis(50);
const WITHIN: Duration = Duration::from_secs(2);

/// One Reticulum packet carrying `data`, framed for the HDLC stream the
/// clients read.
fn framed_packet(data: &[u8]) -> Vec<u8> {
    let packet = Packet { data: PacketDataBuffer::new_from_slice(data), ..Packet::default() };
    let mut bytes = vec![0_u8; 1024];
    let encoded_len = {
        let mut output = OutputBuffer::new(&mut bytes);
        packet.serialize(&mut output).expect("serialize packet");
        output.offset()
    };
    bytes.truncate(encoded_len);
    Hdlc::frame(&bytes).expect("frame packet")
}

#[deny(clippy::await_holding_invalid_type)]
async fn recv_within<T>(what: &str, rx: &mut mpsc::Receiver<T>) -> T {
    timeout(WITHIN, rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|| panic!("{what} channel closed"))
}

/// The client task has ended, so every sender it held is gone: the channel
/// closes with nothing queued on it.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_closed_without_event<T: std::fmt::Debug + PartialEq>(
    what: &str,
    rx: &mut mpsc::Receiver<T>,
) {
    assert_eq!(
        timeout(WITHIN, rx.recv()).await.unwrap_or_else(|_| panic!("{what} stayed open")),
        None,
        "{what} fired for a stream the client ended itself"
    );
}

#[deny(clippy::await_holding_invalid_type)]
async fn join_within(task: tokio::task::JoinHandle<()>, what: &str) {
    timeout(WITHIN, task)
        .await
        .unwrap_or_else(|_| panic!("{what} timed out"))
        .unwrap_or_else(|err| panic!("{what} panicked: {err}"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn tcp_client_whose_receive_queue_closed_fires_the_plain_channel_but_no_reason() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("listener address");
    let (plain_tx, mut plain_rx) = mpsc::channel::<AddressHash>(4);
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = TcpClient::new(addr.to_string())
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_events(plain_tx)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(TcpClient::spawn(context));

    // Positive control: a peer that drops its end is reported on both
    // channels, so the reason channel is known to be live.
    let (first, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("first");
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );
    assert_eq!(recv_within("plain disconnect", &mut plain_rx).await, iface);

    let (mut second, _) =
        timeout(WITHIN, listener.accept()).await.expect("accept").expect("second");
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);

    // The transport side goes away while the stream is up; the next packet
    // the client decodes has nowhere to go and the stream ends.
    drop(manager);
    second.write_all(&framed_packet(b"nowhere-to-deliver")).await.expect("write packet");

    let (third, _) = timeout(WITHIN, listener.accept())
        .await
        .expect("the client did not dial again after its receive queue closed")
        .expect("third");
    assert_eq!(recv_within("third connect event", &mut connect_rx).await, iface);
    assert_eq!(
        recv_within("plain disconnect for the receive-queue-closed stream", &mut plain_rx).await,
        iface,
        "the plain channel reports every ended stream, this one included"
    );
    assert_eq!(
        reason_rx.try_recv(),
        Err(TryRecvError::Empty),
        "a stream the client ended itself has no reason"
    );

    cancel.cancel();
    drop(listener);
    join_within(task, "TCP client task").await;
    drop(second);
    drop(third);
    assert_closed_without_event("the reason channel", &mut reason_rx).await;
}

#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn local_unix_client_whose_receive_queue_closed_fires_the_plain_channel_but_no_reason() {
    let dir = tempfile::tempdir().expect("socket dir");
    let socket = dir.path().join("receive-queue.sock");
    let listener = UnixListener::bind(&socket).expect("bind unix listener");
    let (plain_tx, mut plain_rx) = mpsc::channel::<AddressHash>(4);
    let (reason_tx, mut reason_rx) = mpsc::channel(4);
    let (connect_tx, mut connect_rx) = mpsc::channel::<AddressHash>(4);
    let client = LocalUnixClient::new_connect(LocalUnixEndpoint::filesystem(&socket))
        .with_reconnect_wait(RECONNECT_WAIT)
        .with_connect_events(connect_tx)
        .with_disconnect_events(plain_tx)
        .with_disconnect_reason_events(reason_tx);

    let mut manager = InterfaceManager::new(8);
    let context = manager.new_context(client);
    let iface = context.channel.address;
    let cancel = context.cancel.clone();
    let task = tokio::spawn(LocalUnixClient::spawn(context));

    let (first, _) = timeout(WITHIN, listener.accept()).await.expect("accept").expect("first");
    assert_eq!(recv_within("first connect event", &mut connect_rx).await, iface);
    drop(first);
    assert_eq!(
        recv_within("peer-closed reason", &mut reason_rx).await,
        (iface, DisconnectReason::PeerClosed)
    );
    assert_eq!(recv_within("plain disconnect", &mut plain_rx).await, iface);

    let (mut second, _) =
        timeout(WITHIN, listener.accept()).await.expect("accept").expect("second");
    assert_eq!(recv_within("second connect event", &mut connect_rx).await, iface);

    drop(manager);
    second.write_all(&framed_packet(b"nowhere-to-deliver")).await.expect("write packet");

    let (third, _) = timeout(WITHIN, listener.accept())
        .await
        .expect("the client did not dial again after its receive queue closed")
        .expect("third");
    assert_eq!(recv_within("third connect event", &mut connect_rx).await, iface);
    assert_eq!(
        recv_within("plain disconnect for the receive-queue-closed stream", &mut plain_rx).await,
        iface,
        "the plain channel reports every ended stream, this one included"
    );
    assert_eq!(
        reason_rx.try_recv(),
        Err(TryRecvError::Empty),
        "a stream the client ended itself has no reason"
    );

    cancel.cancel();
    join_within(task, "unix client task").await;
    drop(second);
    drop(third);
    assert_closed_without_event("the reason channel", &mut reason_rx).await;
}
