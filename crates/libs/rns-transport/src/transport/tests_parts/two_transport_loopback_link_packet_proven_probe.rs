// Usage-probe coverage for `LinkEvent::PacketProven` and `DeliveryReceipt::link_id`, from the
// consumer's side of the API: several plain Data packets in flight at once each earn exactly
// one `PacketProven` carrying their own hash (and one `DeliveryReceipt` naming the link); a
// plain Resource sent on the link and a request that travels as a Resource earn none on
// either side, even though both are proved by the Resource machinery (`RNS/Link.py:950-962`
// proves only `context == NONE`); and a proof for a packet that never travelled on a link
// reaches the `ReceiptHandler` with `link_id() == None`. Helpers come from the sibling
// `tests_parts` files.

/// How long the probe listens for a `PacketProven` that must not arrive.
const PACKET_PROVEN_PROBE_QUIET: Duration = Duration::from_millis(500);

/// Collects `PacketProven` hashes for `link_id` until `count` have arrived.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe_collect(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    count: usize,
) -> Vec<Hash> {
    let mut seen = Vec::with_capacity(count);
    timeout(LOOPBACK_TIMEOUT, async {
        while seen.len() < count {
            match events.recv().await {
                Ok(event) if event.id == link_id => {
                    if let Some(hash) = packet_proven_hash(&event.event) {
                        seen.push(hash);
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out: {} of {count} PacketProven arrived", seen.len()));
    seen
}

/// Lets the quiet window pass, then counts the `PacketProven` events for `link_id` that
/// arrived on `events` meanwhile.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe_count_after_quiet(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
) -> usize {
    tokio::time::sleep(PACKET_PROVEN_PROBE_QUIET).await;
    packet_proven_count(events, link_id)
}

/// Three plain Data packets sent back to back are each proved once: the three
/// `PacketProven` hashes are exactly the three sent hashes, no fourth arrives, and the
/// receipt handler sees the same three proofs, every one naming the link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe_once_per_packet_in_a_burst() {
    let mut pair = loopback_request_pair("packet-proven-burst").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;

    let mut sent = Vec::new();
    for body in [&b"one"[..], &b"two"[..], &b"three"[..]] {
        sent.push(
            packet_proven_send_plain_data(
                &pair.handles.connector.transport,
                &pair.handles.link,
                body,
            )
            .await,
        );
    }
    assert_eq!(sent.iter().collect::<HashSet<_>>().len(), 3, "every packet has its own hash");

    let proven = packet_proven_probe_collect(&mut pair.out_events, link_id, 3).await;
    assert_eq!(
        proven.iter().collect::<HashSet<_>>(),
        sent.iter().collect::<HashSet<_>>(),
        "PacketProven hashes are exactly the sent hashes: proven {proven:?}, sent {sent:?}"
    );
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "no packet is proved twice"
    );

    let receipts = receipts.lock().expect("recording lock");
    assert_eq!(receipts.len(), 3, "one receipt per proof: {receipts:?}");
    assert_eq!(
        receipts.iter().map(DeliveryReceipt::packet_hash).collect::<HashSet<_>>(),
        sent.iter().copied().collect::<HashSet<_>>(),
    );
    assert!(
        receipts.iter().all(|receipt| receipt.link_id() == Some(link_id)),
        "every link-packet receipt names the link: {receipts:?}"
    );
}

/// A plain Resource sent on the link (connector to listener) and a request that goes out as
/// a Resource (with its small packet response back) are proved by the Resource machinery,
/// not by `PacketProven`: neither side's link stream posts one, and the connector's
/// `ReceiptHandler` sees nothing at all, a Resource proof going to that machinery and the
/// packet response never being proved.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe_none_for_resources_either_side() {
    let mut pair = loopback_request_pair("packet-proven-resources").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    register_handler(&pair, "/small", RequestAllow::All, |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::from("small"))))
    })
    .await;
    let mut sender_resources = pair.handles.connector.transport.resource_events();
    let mut receiver_resources = pair.handles.listener.transport.resource_events();

    // A plain (non-request) Resource, connector -> listener.
    let payload = random_payload(300 * 1024);
    let resource_hash = pair
        .handles
        .connector
        .transport
        .send_resource(&link_id, payload.clone(), None)
        .await
        .expect("resource dispatched");
    let (complete, _) = await_complete_with_segments(&mut receiver_resources, link_id).await;
    assert_eq!(complete.hash, resource_hash);
    assert!(complete_payload(complete).data == payload, "listener assembled the payload");
    let outbound = await_resource_event(&mut sender_resources, |event| {
        event.hash == resource_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
                    | ResourceEventKind::OutboundRejected { .. }
            )
    })
    .await;
    assert!(
        matches!(outbound.kind, ResourceEventKind::OutboundComplete),
        "the plain Resource is proved complete: {:?}",
        outbound.kind
    );

    // A request that cannot fit a packet, answered by a packet.
    let body = over_mdu_body(&pair).await;
    let receipt = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    let response = await_receipt(&receipt).await.expect("the resource-form request is answered");
    assert_eq!(response.value(), Some(&rmpv::Value::from("small")));

    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "no PacketProven on the connector's link stream for Resource traffic"
    );
    assert_eq!(
        packet_proven_count(&mut pair.handles.in_events, link_id),
        0,
        "no PacketProven on the listener's link stream for Resource traffic"
    );
    let receipts = receipts.lock().expect("recording lock");
    assert!(
        receipts.is_empty(),
        "no DeliveryReceipt reaches the connector for Resource traffic: {receipts:?}"
    );
}

/// A delivery proof for a plain single-destination packet, which never travelled on a link,
/// reaches the `ReceiptHandler` with the proved hash and `link_id() == None`. Same shape as
/// `proof_strategy_all_generates_a_valid_delivery_proof`, read through the new getter.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn delivery_receipt_for_a_single_destination_proof_has_no_link_id() {
    let receiver_identity = PrivateIdentity::new_from_rand(OsRng);
    let receiver_transport =
        Transport::new(TransportConfig::new("receiver", &receiver_identity, true));
    let mut receiver_iface = receiver_transport.iface_manager().lock().await.new_channel(8);
    let own_destination = receiver_transport
        .add_destination(receiver_identity.clone(), DestinationName::new("probe", "proof"))
        .await;
    let own_hash = own_destination.lock().await.desc.address_hash;
    own_destination.lock().await.set_proof_strategy(ProofStrategy::All);

    let sender_identity = PrivateIdentity::new_from_rand(OsRng);
    let sender_transport = Transport::new(TransportConfig::new("sender", &sender_identity, true));
    let mut sender_iface = sender_transport.iface_manager().lock().await.new_channel(8);
    let receiver_announce =
        own_destination.lock().await.announce(OsRng, None).expect("valid announce");
    handle_announce(
        &receiver_announce,
        sender_transport.get_handler().lock().await,
        sender_iface.address,
        crate::iface::IfaceSource::None,
    )
    .await;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    sender_transport.set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone()))).await;

    let outbound = Packet {
        destination: own_hash,
        data: PacketDataBuffer::new_from_slice(b"no link here"),
        ..Packet::default()
    };
    assert_eq!(
        sender_transport.send_packet_with_trace(outbound).await.outcome,
        SendPacketOutcome::SentDirect
    );
    let sent = timeout(Duration::from_millis(200), sender_iface.tx_channel.recv())
        .await
        .expect("packet queued on the sender's interface")
        .expect("tx channel open");
    let sent_hash = sent.packet.hash();
    handle_data(
        &sent.packet,
        receiver_iface.address,
        receiver_transport.get_handler().lock().await,
    )
    .await;
    let proof = timeout(Duration::from_millis(200), receiver_iface.tx_channel.recv())
        .await
        .expect("a delivery proof comes back")
        .expect("tx channel open");
    assert_eq!(proof.packet.header.packet_type, PacketType::Proof);

    sender_transport.handle_inbound_for_test(proof.packet).await;
    let receipts = receipts.lock().expect("recording lock");
    assert_eq!(receipts.len(), 1, "exactly one receipt: {receipts:?}");
    assert_eq!(receipts[0].packet_hash(), sent_hash);
    assert_eq!(receipts[0].link_id(), None, "a single-destination proof names no link");
}
