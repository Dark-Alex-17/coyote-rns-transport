// Loopback coverage for the resource-retry worker. A peer that cannot process traffic for a
// while makes the sender re-advertise and the receiver re-request. Both retries address the
// Link, so over broadcast-off they only arrive if they leave on the link's bound interface;
// the stalled peer's `iface_rx` tap counts what actually landed, and the peer must then treat
// the duplicate it receives as a no-op. Helpers come from the sibling `tests_parts` files.

/// The default retry budget is 2 s and the worker polls every 2 s, so a retry is due at most
/// 4 s after the first attempt. The stall covers one guaranteed retry with margin and stays
/// well inside both the four-advertisement budget and the link's keepalive window.
const RETRY_STALL: Duration = Duration::from_secs(5);
/// Small enough that the whole transfer is a handful of packets, so the broadcast tap
/// (capacity 1024) cannot lag and every packet that landed is counted.
const SMALL_RESOURCE_LEN: usize = 4 * 1024;

/// Holds `handler`'s lock for `stall`, which blocks that transport's inbound processing and
/// its own retry worker while the peer's workers keep running. Signals `stalled` once the
/// lock is held. Runs on a blocking thread so the guard is held across a plain sleep, never
/// across an await.
fn stall_handler(
    handler: Arc<Mutex<TransportHandler>>,
    stalled: tokio::sync::oneshot::Sender<()>,
    stall: Duration,
) {
    let _guard = handler.blocking_lock();
    let _ = stalled.send(());
    std::thread::sleep(stall);
}

/// Spawns a `RETRY_STALL` stall against `transport` and returns once the lock is held.
#[deny(clippy::await_holding_invalid_type)]
async fn stall_transport(transport: &Transport) -> tokio::task::JoinHandle<()> {
    stall_transport_for(transport, RETRY_STALL).await
}

/// Spawns a stall of `stall` against `transport` and returns once the lock is held.
#[deny(clippy::await_holding_invalid_type)]
async fn stall_transport_for(
    transport: &Transport,
    stall: Duration,
) -> tokio::task::JoinHandle<()> {
    let (stalled_tx, stalled_rx) = tokio::sync::oneshot::channel();
    let handler = transport.get_handler();
    let stall = tokio::task::spawn_blocking(move || stall_handler(handler, stalled_tx, stall));
    stalled_rx.await.expect("stall task acquired the handler lock");
    stall
}

/// Counts packets for `link_id` with `context` that reached the tapped interface. The tap
/// fires before any handler lock, so a packet counts as soon as it is read off the wire. A
/// lagged tap would make the count meaningless, so it fails the test instead.
fn count_link_packets(
    tap: &mut broadcast::Receiver<RxMessage>,
    link_id: AddressHash,
    context: PacketContext,
) -> usize {
    let mut count = 0;
    loop {
        match tap.try_recv() {
            Ok(message) => {
                if message.packet.destination == link_id && message.packet.context == context {
                    count += 1;
                }
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                panic!("interface tap lagged by {skipped} packets; counts are unreliable")
            }
            Err(_) => return count,
        }
    }
}

/// One receiver-side `Complete` (single segment, so no `SegmentComplete`), the payload, and
/// the sender's clean terminal event for a small transfer dispatched as `resource_hash`.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_small_transfer_completes(
    sender_events: &mut broadcast::Receiver<ResourceEvent>,
    receiver_events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    resource_hash: Hash,
    payload: &[u8],
) {
    let (complete, segments_seen) = await_complete_with_segments(receiver_events, link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments_seen, None, "a single-segment transfer emits no SegmentComplete");
    let complete = complete_payload(complete);
    assert_eq!(complete.data.len(), payload.len(), "received length");
    assert!(complete.data == payload, "received bytes match the sent payload");

    let outbound = await_resource_event(sender_events, |event| {
        event.hash == resource_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
                    | ResourceEventKind::OutboundRejected
            )
    })
    .await;
    assert!(
        matches!(outbound.kind, ResourceEventKind::OutboundComplete),
        "sender should finish cleanly, got {:?}",
        outbound.kind
    );
    assert_no_further_complete(receiver_events, link_id).await;
}

/// Advertisement retry from the initiator (out-link) side. The listener is stalled before
/// the connector advertises, so the first advertisement sits unprocessed and the connector's
/// retry worker re-advertises. The retried advertisement must land on the listener's
/// interface (it only can via `Direct(link iface)`; the initiator's link is not in the path
/// table), and once the listener wakes the duplicate must not disturb the transfer: one
/// `Complete`, the right bytes, `OutboundComplete` on the connector.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_initiator_advertisement_retry_lands_once() {
    let pair = loopback_link_pair("adv-retry").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();
    let mut listener_tap = pair.listener.transport.iface_rx();

    let stall = stall_transport(&pair.listener.transport).await;
    let payload = random_payload(SMALL_RESOURCE_LEN);
    let resource_hash = pair
        .connector
        .transport
        .send_resource(&pair.link_id, payload.clone(), None)
        .await
        .expect("resource dispatched from the initiator side");
    stall.await.expect("stall task finished");

    assert_small_transfer_completes(
        &mut sender_events,
        &mut receiver_events,
        pair.link_id,
        resource_hash,
        &payload,
    )
    .await;

    let advertisements =
        count_link_packets(&mut listener_tap, pair.link_id, PacketContext::ResourceAdvrtisement);
    assert!(
        advertisements >= 2,
        "the retried advertisement must reach the listener's interface; saw {advertisements} \
         advertisement(s) for the link"
    );
}

/// Part-request retry from the initiator (out-link) side. The listener advertises a response
/// and is stalled before the advertisement even leaves its interface queue, so the
/// connector's first request goes unanswered and its retry worker re-requests. The retried
/// request must land on the listener's interface, and the duplicate request the listener then
/// serves must not produce a second `Complete` on the connector.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_initiator_request_retry_lands_once() {
    let pair = loopback_link_pair("req-retry").await;
    let mut sender_events = pair.listener.transport.resource_events();
    let mut receiver_events = pair.connector.transport.resource_events();
    let mut listener_tap = pair.listener.transport.iface_rx();

    let payload = random_payload(SMALL_RESOURCE_LEN);
    let request_id = b"loopback-retry".to_vec();
    // The soliciting request completes before the listener is stalled, so the stall holds
    // back only the response advertisement. `RETRY_STALL` is well under the default request
    // timeout (`rtt * 6 + 11.25 s`), so the record is still armed when the advertisement
    // lands; the stall past it is `two_transport_loopback_resources_receipt_less_receiving.rs`'s.
    let request_hash =
        solicit_response(&pair.connector.transport, &pair.link_id, &request_id).await;
    await_resource_event(&mut receiver_events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
    let resource_hash = pair
        .listener
        .transport
        .send_response_resource(&pair.link_id, request_id.clone(), payload.clone(), None)
        .await
        .expect("response resource dispatched");
    // `send_response_resource` only queues the advertisement; the stall task is woken here,
    // before the interface writer gets a turn, so the lock is held before the connector can
    // answer.
    let stall = stall_transport(&pair.listener.transport).await;
    stall.await.expect("stall task finished");

    assert_small_transfer_completes(
        &mut sender_events,
        &mut receiver_events,
        pair.link_id,
        resource_hash,
        &payload,
    )
    .await;

    let requests =
        count_link_packets(&mut listener_tap, pair.link_id, PacketContext::ResourceRequest);
    assert!(
        requests >= 2,
        "the retried part request must reach the listener's interface; saw {requests} \
         request(s) for the link"
    );
}
