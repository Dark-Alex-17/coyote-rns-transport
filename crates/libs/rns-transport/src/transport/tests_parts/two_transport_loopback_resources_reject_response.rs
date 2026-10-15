// Loopback coverage for the other advertisement reject branch: a response Resource larger
// than the limit the requester attached to its request. Here the reject is sent by the
// CONNECTOR, from inside its inbound handler for an initiator-side link, so the test checks
// that side of the transport is still alive afterwards: the same link carries a second
// response end to end and the connector's interface can still be stopped. Helpers and the
// reject-side constants come from the sibling `tests_parts` files included into this module.

/// The bounded request carries 4 KiB of random bytes, so bz2 cannot bring the response's
/// transfer size back under the limit the requester attached.
const OVERSIZE_RESPONSE_LEN: usize = 4 * 1024;
const ACCEPTED_RESPONSE_LEN: usize = 256;
const BOUNDED_REQUEST_LEN: usize = 64;

/// A requester bounds the response it is willing to accept. The responder's oversize
/// response must be rejected with a receiver cancel that the connector sends from inside
/// the link's inbound handler; the responder must learn of the rejection, the same link must
/// then carry an accepted response Resource, and the connector's interface must still stop
/// within a bounded time.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_reject_oversize_response_then_recover_on_same_link()
{
    let pair = loopback_link_pair("reject-response").await;
    let mut requester_events = pair.connector.transport.resource_events();
    let mut responder_events = pair.listener.transport.resource_events();
    let mut connector_tap = pair.connector.transport.iface_rx();

    // The requester attaches a response limit to its request.
    let bounded_request_id = b"loopback-bounded".to_vec();
    let request_hash = pair
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &pair.link_id,
            bounded_request_id.clone(),
            random_payload(BOUNDED_REQUEST_LEN),
            None,
            Some(REJECT_REQUEST_LIMIT),
        )
        .await
        .expect("bounded request resource dispatched");
    let request = await_resource_event(&mut responder_events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let request = complete_payload(request);
    assert!(request.is_request, "advertised as a request");
    assert_eq!(request.request_id.as_deref(), Some(bounded_request_id.as_slice()));

    // The responder answers with more than the requester allowed.
    let oversize = random_payload(OVERSIZE_RESPONSE_LEN);
    let rejected_hash = pair
        .listener
        .transport
        .send_response_resource(&pair.link_id, bounded_request_id.clone(), oversize, None)
        .await
        .expect("oversize response resource dispatched");
    let rejected = await_resource_event(&mut responder_events, |event| {
        event.hash == rejected_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(rejected.kind, ResourceEventKind::OutboundRejected),
        "the oversize response is rejected, got {:?}",
        rejected.kind
    );

    // The same link still carries a response the requester did not bound.
    let payload = random_payload(ACCEPTED_RESPONSE_LEN);
    let accepted_request_id = b"loopback-unbounded".to_vec();
    let resource_hash = pair
        .listener
        .transport
        .send_response_resource(&pair.link_id, accepted_request_id.clone(), payload.clone(), None)
        .await
        .expect("accepted response resource dispatched");
    let complete = await_resource_event(&mut requester_events, |event| {
        event.hash == resource_hash && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    assert_eq!(complete.link_id, pair.link_id, "delivered on the link that saw the reject");
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(accepted_request_id.as_slice()));
    assert!(complete.data == payload, "received bytes match the sent payload");
    let outbound = await_resource_event(&mut responder_events, |event| {
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
        "the accepted response finishes cleanly, got {:?}",
        outbound.kind
    );

    // The rejecting side's handler is not wedged: its interface stops in bounded time.
    let iface = tapped_link_iface(&mut connector_tap, pair.link_id);
    let stopped = timeout(STOP_INTERFACE_TIMEOUT, pair.connector.transport.stop_interface(iface))
        .await
        .expect("the connector's handler is not wedged after the reject");
    assert!(stopped, "the tapped interface was known to the connector");
}
