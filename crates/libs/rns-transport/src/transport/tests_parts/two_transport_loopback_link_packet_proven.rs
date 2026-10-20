// Loopback coverage for `LinkEvent::PacketProven`: the peer proves a plain Data packet sent
// on the link (`RNS/Link.py:950-962`), and the proof consumes the hash the sender recorded
// at send time, the way Python's proof removes the one `SENT` receipt it validates
// (`RNS/Transport.py:2748-2761`): one event on the sender's link event stream, from either
// side of the link, and one `DeliveryReceipt` naming the link at the transport
// `ReceiptHandler`. A request, its response and a response Resource post none. Helpers come
// from the sibling `tests_parts` files.

use tokio::sync::broadcast::error::TryRecvError;

/// Records every receipt a transport hands out.
struct PacketProvenRecording(Arc<StdMutex<Vec<DeliveryReceipt>>>);

impl ReceiptHandler for PacketProvenRecording {
    fn on_receipt(&self, receipt: &DeliveryReceipt) {
        self.0.lock().expect("recording lock").push(*receipt);
    }
}

fn packet_proven_hash(event: &LinkEvent) -> Option<Hash> {
    match event {
        LinkEvent::PacketProven { packet_hash } => Some(*packet_hash),
        _ => None,
    }
}

/// Builds a plain Data packet on `link`, sends it on the link's bound interface and returns
/// its hash. Link packets are not re-encrypted on the way out, so the hash is the one the
/// peer proves.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_send_plain_data(
    transport: &Transport,
    link: &Arc<Mutex<Link>>,
    data: &[u8],
) -> Hash {
    let packet = link.lock().await.data_packet(data).expect("plain data fits a packet");
    let hash = packet.hash();
    let outcome = transport.send_link_packet_on_bound_iface(link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "plain data reaches the wire");
    hash
}

/// Drains `events` and counts the `PacketProven` events for `link_id` among them.
fn packet_proven_count(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
) -> usize {
    let mut count = 0;
    loop {
        match events.try_recv() {
            Ok(event) => {
                if event.id == link_id && packet_proven_hash(&event.event).is_some() {
                    count += 1;
                }
            }
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty | TryRecvError::Closed) => break,
        }
    }
    count
}

/// The connector sends one plain Data packet, then a packet request and a request answered
/// by a Resource. Only the plain packet is proved: one `PacketProven` with its hash on the
/// link's event stream, and one `DeliveryReceipt` naming the link at the receipt handler.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_once_per_plain_data_packet() {
    let mut pair = loopback_request_pair("packet-proven").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let hash = packet_proven_send_plain_data(
        &pair.handles.connector.transport,
        &pair.handles.link,
        b"prove me",
    )
    .await;
    await_event(&mut pair.out_events, |event| {
        event.id == link_id && packet_proven_hash(&event.event) == Some(hash)
    })
    .await;

    let echoed = request(&pair, "/echo", rmpv::Value::from(1)).await;
    await_receipt(&echoed).await.expect("the echo request is answered");
    let big = request(&pair, "/big", rmpv::Value::Nil).await;
    await_receipt(&big).await.expect("the Resource response arrives");

    assert_eq!(
        packet_proven_count(&mut pair.out_events, link_id),
        0,
        "the awaited PacketProven is the only one in the whole exchange"
    );
    let receipts = receipts.lock().expect("recording lock");
    assert_eq!(receipts.len(), 1, "one proof reaches the receipt handler: {receipts:?}");
    assert_eq!(receipts[0].packet_hash(), hash);
    assert_eq!(receipts[0].link_id(), Some(link_id));
}

/// The responder's in-link posts `PacketProven` for a plain Data packet it sends, once the
/// connector has received and proved it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_on_the_responder_side_too() {
    let mut pair = loopback_request_pair("packet-proven-responder").await;
    let link_id = pair.handles.link_id;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("responder tracks the inbound link");

    let hash =
        packet_proven_send_plain_data(&pair.handles.listener.transport, &in_link, b"pong").await;

    let received = await_event(&mut pair.out_events, |event| {
        event.id == link_id && is_plain_data(&event.event)
    })
    .await;
    assert_eq!(data_bytes(received), b"pong");
    await_event(&mut pair.handles.in_events, |event| {
        event.id == link_id && packet_proven_hash(&event.event) == Some(hash)
    })
    .await;
}

#[test]
fn delivery_receipt_new_has_no_link_id() {
    assert!(DeliveryReceipt::new([7u8; 32]).link_id().is_none());
}
