// Usage-probe coverage for the sent-hash gate behind `LinkEvent::PacketProven`, exercised
// through the transport send seam over loopback rather than by recording hashes on a `Link`
// by hand: the record keeps the latest `UNPROVEN_DATA_HASHES_CAP` plain Data hashes, oldest
// dropped first, so a burst past the cap sent while the peer is stalled earns exactly the cap
// in `PacketProven` events, for the newest hashes, while every proof still reaches the
// transport `ReceiptHandler` as a `DeliveryReceipt` naming the link; and a Channel frame sent
// through `Transport::channel` is proved by the peer (`RNS/Link.py:1147-1151`) and marked
// `Delivered`, reaches the `ReceiptHandler`, but posts no `PacketProven` because its hash is
// never recorded. Helpers come from the sibling `tests_parts` files.

/// How many plain Data packets past the cap the burst sends.
const PACKET_PROVEN_PROBE2_OVER_CAP: usize = 3;
/// Longest gap allowed between two consecutive proofs (or receipts) of the burst. The
/// collection waits on progress, not on a fixed budget: a slow runner stretches the run
/// without weakening the count, and only a stall of this long fails the test. The whole
/// burst took longer than `LOOPBACK_TIMEOUT` on a CI runner once.
const PACKET_PROVEN_PROBE2_PROGRESS_WINDOW: Duration = LOOPBACK_TIMEOUT;

/// Holds `handler`'s lock until `release` fires or its sender drops, signalling `stalled`
/// once the lock is held. Runs on a blocking thread so the guard never spans an await.
fn stall_handler_until_released(
    handler: Arc<Mutex<TransportHandler>>,
    stalled: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
) {
    let _guard = handler.blocking_lock();
    let _ = stalled.send(());
    let _ = release.blocking_recv();
}

/// Spawns a stall against `transport` that lasts until the returned sender fires, and
/// returns once the lock is held. The burst must be recorded in full before the first proof
/// is matched; a fixed-length stall raced the sends on a loaded runner and the peer proved
/// early hashes before the cap could drop them. The shared-instance fixtures also use it to
/// hold the handler while an attach or a stop is mid-flight, so the slot can be observed
/// between the two sections of the call.
#[deny(clippy::await_holding_invalid_type)]
async fn stall_transport_until_released(
    transport: &Transport,
) -> (tokio::task::JoinHandle<()>, tokio::sync::oneshot::Sender<()>) {
    let (stalled_tx, stalled_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let handler = transport.get_handler();
    let stall = tokio::task::spawn_blocking(move || {
        stall_handler_until_released(handler, stalled_tx, release_rx)
    });
    stalled_rx.await.expect("stall task acquired the handler lock");
    (stall, release_tx)
}

/// Collects `PacketProven` hashes for `link_id` until `count` have arrived, failing when no
/// new proof lands within `PACKET_PROVEN_PROBE2_PROGRESS_WINDOW` of the previous one. A
/// lagged event stream would make the count meaningless, so it fails the test instead.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe2_collect_exact(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    count: usize,
) -> Vec<Hash> {
    let mut seen = Vec::with_capacity(count);
    let mut deadline = Instant::now() + PACKET_PROVEN_PROBE2_PROGRESS_WINDOW;
    while seen.len() < count {
        let received =
            tokio::time::timeout_at(deadline, events.recv()).await.unwrap_or_else(|_| {
                panic!(
                    "stalled: {} of {count} PacketProven arrived, none in the last {:?}",
                    seen.len(),
                    PACKET_PROVEN_PROBE2_PROGRESS_WINDOW
                )
            });
        match received {
            Ok(event) if event.id == link_id => {
                if let Some(hash) = packet_proven_hash(&event.event) {
                    seen.push(hash);
                    deadline = Instant::now() + PACKET_PROVEN_PROBE2_PROGRESS_WINDOW;
                }
            }
            Ok(_) => {}
            Err(RecvError::Lagged(skipped)) => {
                panic!("link event stream lagged by {skipped}; counts are unreliable")
            }
            Err(RecvError::Closed) => panic!("link event channel closed"),
        }
    }
    seen
}

/// Waits until the recording receipt handler holds `count` receipts, failing when the count
/// does not grow within `PACKET_PROVEN_PROBE2_PROGRESS_WINDOW`.
#[deny(clippy::await_holding_invalid_type)]
async fn packet_proven_probe2_await_receipts(
    receipts: &Arc<StdMutex<Vec<DeliveryReceipt>>>,
    count: usize,
) -> Vec<DeliveryReceipt> {
    let mut last_len = 0;
    let mut deadline = Instant::now() + PACKET_PROVEN_PROBE2_PROGRESS_WINDOW;
    loop {
        let seen = receipts.lock().expect("recording lock").clone();
        if seen.len() >= count {
            return seen;
        }
        if seen.len() > last_len {
            last_len = seen.len();
            deadline = Instant::now() + PACKET_PROVEN_PROBE2_PROGRESS_WINDOW;
        }
        assert!(
            Instant::now() < deadline,
            "stalled: {last_len} of {count} receipts arrived, none in the last {:?}",
            PACKET_PROVEN_PROBE2_PROGRESS_WINDOW
        );
        tokio::time::sleep(LOOPBACK_POLL).await;
    }
}

/// A burst of `crate::destination::link::UNPROVEN_DATA_HASHES_CAP + 3` plain Data packets sent while the peer cannot
/// answer: once the peer catches up, exactly `crate::destination::link::UNPROVEN_DATA_HASHES_CAP` `PacketProven` events
/// arrive, their hashes are the newest `crate::destination::link::UNPROVEN_DATA_HASHES_CAP` sent, the three oldest are
/// never proven and no further event follows; the receipt handler still sees every one of the
/// proofs, each naming the link, because the gate is on the link event, not on the receipt.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe2_cap_drops_the_oldest_at_the_seam() {
    let mut pair = loopback_request_pair("packet-proven-cap").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    let total = crate::destination::link::UNPROVEN_DATA_HASHES_CAP + PACKET_PROVEN_PROBE2_OVER_CAP;

    let (stall, release) = stall_transport_until_released(&pair.handles.listener.transport).await;
    let mut sent = Vec::with_capacity(total);
    for i in 0..total {
        let payload = format!("cap-{i}");
        sent.push(
            packet_proven_send_plain_data(
                &pair.handles.connector.transport,
                &pair.handles.link,
                payload.as_bytes(),
            )
            .await,
        );
    }
    assert_eq!(
        sent.iter().collect::<std::collections::HashSet<_>>().len(),
        total,
        "every packet in the burst has its own hash"
    );
    release.send(()).expect("stall task is waiting for the release");
    stall.await.expect("stall task finishes");

    // The link event channel holds 1024 entries (`TRANSPORT_EVENT_CHANNEL_CAPACITY` in
    // `transport/core.rs`), the same number as the cap: a burst of events for a larger cap
    // would lag this receiver, and the collector treats a lag as a failure.
    let proven = packet_proven_probe2_collect_exact(
        &mut pair.out_events,
        link_id,
        crate::destination::link::UNPROVEN_DATA_HASHES_CAP,
    )
    .await;
    let proven_set: std::collections::HashSet<Hash> = proven.iter().copied().collect();
    let newest: std::collections::HashSet<Hash> =
        sent[PACKET_PROVEN_PROBE2_OVER_CAP..].iter().copied().collect();
    assert_eq!(proven_set, newest, "the newest cap-many hashes are the ones proven");
    for dropped in &sent[..PACKET_PROVEN_PROBE2_OVER_CAP] {
        assert!(!proven_set.contains(dropped), "an oldest hash past the cap was proven");
    }

    let receipts = packet_proven_probe2_await_receipts(&receipts, total).await;
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "no PacketProven follows the cap-many, not even for the dropped hashes"
    );
    assert_eq!(receipts.len(), total, "every proof reaches the receipt handler");
    let receipt_hashes: std::collections::HashSet<Hash> =
        receipts.iter().map(|receipt| receipt.packet_hash()).collect();
    assert_eq!(
        receipt_hashes,
        sent.iter().copied().collect::<std::collections::HashSet<Hash>>(),
        "the receipts name every sent hash, dropped ones included"
    );
    assert!(receipts.iter().all(|receipt| receipt.link_id() == Some(link_id)));
}

/// A Channel frame sent through `Transport::channel` is proved by the peer and marked
/// `Delivered`, and the proof reaches the receipt handler naming the link, but no
/// `PacketProven` is posted for it on either side.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_packet_proven_probe2_none_for_a_channel_frame() {
    let mut pair = loopback_request_pair("packet-proven-channel").await;
    let link_id = pair.handles.link_id;
    let receipts = Arc::new(StdMutex::new(Vec::new()));
    pair.handles
        .connector
        .transport
        .set_receipt_handler(Box::new(PacketProvenRecording(receipts.clone())))
        .await;
    let delivered = Arc::new(StdMutex::new(Vec::new()));
    let seen = delivered.clone();
    pair.handles
        .listener
        .transport
        .channel(link_id)
        .register_handler(0x55AA, move |envelope| {
            seen.lock().expect("delivered lock").push(envelope.payload.clone());
            true
        })
        .await
        .expect("responder registers a channel handler");

    let channel = pair.handles.connector.transport.channel(link_id);
    let sequence = channel.send(0x55AA, b"needs-proof".to_vec()).await.expect("channel send");

    let receipts = packet_proven_probe2_await_receipts(&receipts, 1).await;
    assert_eq!(receipts[0].link_id(), Some(link_id), "the Channel-frame proof names the link");
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            let state = channel.message_state(sequence).await.expect("channel state");
            if state == crate::channel::MessageState::Delivered {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the channel frame is marked Delivered by the proof");
    assert_eq!(
        delivered.lock().expect("delivered lock").as_slice(),
        [b"needs-proof".to_vec()],
        "the responder's channel handler saw the frame"
    );

    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "a Channel frame posts no PacketProven on the sender"
    );
    assert_eq!(
        packet_proven_count(&mut pair.handles.in_events, link_id),
        0,
        "nor on the responder"
    );
}
