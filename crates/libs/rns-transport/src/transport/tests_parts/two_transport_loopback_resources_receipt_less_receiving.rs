// Loopback coverage for a receipt-less solicited response that outlasts the default request
// timeout. The record `send_request_resource*` keeps under an id no `RequestReceipt` owns
// follows a receipt's lifetime rules: its clock runs from the request Resource's delivery
// (`RNS/Link.py:1370-1373`) and stops while a response Resource is inbound (`:1389` never
// times out `RECEIVING`), so the second segment of a split response, advertised more than
// the default request timeout after the request was delivered, is still solicited. A record
// whose clock kept running would have been pruned by the link watchdog by then and the
// advertisement dropped as one nothing on the link sent (`RNS/Link.py:1046-1047`), leaving
// the transfer stuck after its first segment. Helpers come from the sibling `tests_parts`
// files.

/// Headroom past the record's deadline, so the stall ends after a running clock would have
/// expired the record and the watchdog has had several ticks to prune it.
const RECEIPT_LESS_STALL_MARGIN: Duration = Duration::from_secs(3);

/// The two-segment body `two_transport_loopback_resources_two_segments` sends.
const RECEIPT_LESS_RESPONSE_LEN: usize = MAX_EFFICIENT_SIZE + 64 * 1024;

/// A 17-byte id, so the record is the `send_request_resource*` entry alone and no receipt
/// owns it.
const RECEIPT_LESS_REQUEST_ID: &[u8] = b"receipt-less-long";

/// Over `LEGACY_MTU` the first segment is a couple of thousand parts, so a stall placed on
/// the connector's first `Progress` event lands while most of it is still to come and the
/// second segment's advertisement follows the stall. The listener is held for the default
/// request timeout plus margin from the first part; the request Resource was delivered
/// before the response was even sent, so the record's deadline, had it been running, is
/// behind the watchdog when the stall ends.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_receipt_less_receiving_outlasts_the_default_request_timeout(
) {
    let handles = loopback_link_handles_with_mtu("receipt-less-receiving", LEGACY_MTU).await;
    let link_id = handles.link_id;
    let mut sender_events = handles.listener.transport.resource_events();
    let mut receiver_events = handles.connector.transport.resource_events();

    let request_hash =
        solicit_response(&handles.connector.transport, &link_id, RECEIPT_LESS_REQUEST_ID).await;
    await_resource_event(&mut receiver_events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
    let delivered_at = Instant::now();
    let record_timeout = handles.link.lock().await.rtt() * 6 + RESPONSE_GRACE;

    let payload = random_payload(RECEIPT_LESS_RESPONSE_LEN);
    let resource_hash = handles
        .listener
        .transport
        .send_response_resource(&link_id, RECEIPT_LESS_REQUEST_ID.to_vec(), payload.clone(), None)
        .await
        .expect("response resource dispatched");
    await_resource_event(&mut receiver_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Progress(_))
    })
    .await;
    let stall = stall_transport_for(
        &handles.listener.transport,
        record_timeout + RECEIPT_LESS_STALL_MARGIN,
    )
    .await;
    stall.await.expect("stall task finished");
    assert!(
        delivered_at.elapsed() > record_timeout + INTERVAL_LINKS_CHECK,
        "a clock running from the delivery would have expired the record during the stall"
    );

    let (complete, segments_seen) =
        await_complete_with_segments(&mut receiver_events, link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments_seen, Some(2), "both segments were admitted");
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(RECEIPT_LESS_REQUEST_ID));
    assert_eq!(complete.data.len(), payload.len(), "received length");
    assert!(complete.data == payload, "received bytes match the sent payload");

    let outbound = await_resource_event(&mut sender_events, |event| {
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
    assert_no_further_complete(&mut receiver_events, link_id).await;
}
