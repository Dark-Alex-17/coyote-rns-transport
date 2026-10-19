// Usage-probe coverage for the README/CHANGELOG promise that the `DeliveryReceipt` is not
// filtered by the sent-hash record while `LinkEvent::PacketProven` is: a replayed link proof.
// The peer's proof of a plain Data packet is captured off the connector's interface tap and
// fed back in. Byte-identical, it never reaches the link: the ingress packet filter drops it
// the way Python's `Transport.packet_filter` drops any packet hash it has already seen, so
// neither a second receipt nor an event follows. Past that filter (the cache is bounded, so a
// late replay can get there), the proof is validated again and handed to the `ReceiptHandler`
// as a second `DeliveryReceipt` naming the link, but the record no longer holds the hash, so
// no `PacketProven` is posted (`RNS/Transport.py:2748-2761`: the delivery callback fires only
// for an outstanding `SENT` receipt), and a fresh plain Data packet sent afterwards still earns
// exactly one. Helpers come from the sibling `tests_parts` files.

/// Waits on `tap` for the peer's proof of `hash` on `link_id`: a `Proof` packet addressed to
/// the link, `PacketContext::None` on the wire, whose data opens with the proved hash.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe3_capture_proof(
    tap: &mut broadcast::Receiver<RxMessage>,
    link_id: AddressHash,
    hash: Hash,
) -> RxMessage {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match tap.recv().await {
                Ok(message) => {
                    let packet = &message.packet;
                    if packet.header.packet_type == PacketType::Proof
                        && packet.destination == link_id
                        && packet.context == PacketContext::None
                        && packet.data.as_slice().starts_with(hash.as_slice())
                    {
                        return message;
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    panic!("interface tap lagged by {skipped} packets; the proof may be lost")
                }
                Err(RecvError::Closed) => panic!("interface tap closed"),
            }
        }
    })
    .await
    .expect("the peer's proof reached the connector's interface")
}

/// Sends one plain Data packet from the connector, waits for its `PacketProven` and its
/// receipt, and returns the hash and the proof packet as it arrived on the wire.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe3_prove_one(
    pair: &mut LoopbackRequestPair,
    tap: &mut broadcast::Receiver<RxMessage>,
    receipts: &Arc<StdMutex<Vec<DeliveryReceipt>>>,
    body: &[u8],
) -> (Hash, RxMessage) {
    let link_id = pair.handles.link_id;
    let hash =
        packet_proven_send_plain_data(&pair.handles.connector.transport, &pair.handles.link, body)
            .await;
    await_event(&mut pair.out_events, |event| {
        event.id == link_id && packet_proven_hash(&event.event) == Some(hash)
    })
    .await;
    let seen = packet_proven_probe2_await_receipts(receipts, 1).await;
    assert_eq!(seen.last().map(DeliveryReceipt::packet_hash), Some(hash));
    let proof = packet_proven_probe3_capture_proof(tap, link_id, hash).await;
    (hash, proof)
}

/// A byte-identical replay of the peer's proof, offered to the connector's ingress the way an
/// interface would, is refused by the packet filter (its hash was seen when the real proof
/// came in): no second `DeliveryReceipt`, no `PacketProven`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe3_replay_is_dropped_at_ingress() {
    let mut pair = loopback_request_pair("packet-proven-replay-ingress").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    let mut tap = pair.handles.connector.transport.iface_rx();

    let (hash, proof) =
        packet_proven_probe3_prove_one(&mut pair, &mut tap, &receipts, b"prove once").await;

    let transport = &pair.handles.connector.transport;
    let queued = super::inbound_processing::preprocess_inbound_message(
        &transport.get_handler(),
        &transport.iface_messages_tx,
        proof,
    )
    .await;
    assert!(queued.is_none(), "the ingress packet filter refuses a proof hash it already saw");

    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "a replay dropped at ingress posts no PacketProven"
    );
    let receipts = receipts.lock().expect("recording lock");
    assert_eq!(receipts.len(), 1, "nor does it reach the receipt handler: {receipts:?}");
    assert_eq!(receipts[0].packet_hash(), hash);
}

/// The same replay delivered past the packet filter, straight into the connector's inbound
/// processing, is validated and handed out as a second `DeliveryReceipt` naming the link, but
/// the record was consumed by the first proof, so no `PacketProven` follows; a fresh plain
/// Data packet sent afterwards still earns exactly one event with its own hash.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe3_replay_past_ingress_receipt_only() {
    let mut pair = loopback_request_pair("packet-proven-replay-link").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    let mut tap = pair.handles.connector.transport.iface_rx();

    let (hash, proof) =
        packet_proven_probe3_prove_one(&mut pair, &mut tap, &receipts, b"prove once").await;

    let replay = QueuedInbound {
        message: proof,
        ingress_limited: false,
        packet_cache_inserted: false,
        path_request: None,
    };
    super::inbound_processing::process_inbound_message(
        pair.handles.connector.transport.get_handler(),
        replay,
    )
    .await;

    let seen = packet_proven_probe2_await_receipts(&receipts, 2).await;
    assert_eq!(seen.len(), 2, "the replayed proof is handed out as a second receipt: {seen:?}");
    assert!(
        seen.iter().all(|receipt| receipt.packet_hash() == hash),
        "both receipts name the proved packet: {seen:?}"
    );
    assert!(
        seen.iter().all(|receipt| receipt.link_id() == Some(link_id)),
        "both receipts name the link: {seen:?}"
    );
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "the replay finds no recorded hash, so no second PacketProven"
    );

    let fresh = packet_proven_send_plain_data(
        &pair.handles.connector.transport,
        &pair.handles.link,
        b"prove again",
    )
    .await;
    assert_ne!(fresh, hash, "a different body is a different hash");
    await_event(&mut pair.out_events, |event| {
        event.id == link_id && packet_proven_hash(&event.event) == Some(fresh)
    })
    .await;
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "the fresh packet earns exactly one PacketProven"
    );
    let seen = packet_proven_probe2_await_receipts(&receipts, 3).await;
    assert_eq!(seen.len(), 3, "three proofs reached the receipt handler in all: {seen:?}");
    assert_eq!(seen[2].packet_hash(), fresh);
}
