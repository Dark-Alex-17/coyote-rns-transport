// Loopback coverage for the `send_*_with_outcome` entry points: each terminal verdict of an
// outbound Resource must resolve the outcome the sender was handed, with the same answer
// the resource events carry. Helpers come from the sibling `tests_parts` files included
// into this module.

use crate::resource::{OutboundFailure, OutboundResult, RejectReason};

/// The destination hash the connector's one out-link is keyed under, which is what
/// `reset_out_link` takes. The guard is released before the function returns.
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_out_link_destination(transport: &Transport) -> AddressHash {
    let handler = transport.get_handler();
    let handler = handler.lock().await;
    let mut destinations = handler.out_links.keys();
    let destination = *destinations.next().expect("the connector holds an out-link");
    assert!(destinations.next().is_none(), "the connector holds exactly one out-link");
    destination
}

/// A split Resource that the listener assembles resolves `Complete`, and only once the
/// final segment is proven: the receiver has seen all three segments by then.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_complete_for_a_three_segment_send() {
    let pair = loopback_link_pair("outcome-complete").await;
    let mut listener_events = pair.listener.transport.resource_events();

    let payload = random_payload(2 * MAX_EFFICIENT_SIZE + 17);
    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, payload.clone(), None, true)
        .await
        .expect("resource dispatched from the initiator side");

    let (complete, segments_seen) =
        await_complete_with_segments(&mut listener_events, pair.link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments_seen, Some(3), "segment count on the receiver");
    assert!(complete_payload(complete).data == payload, "received bytes match the sent payload");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
}

/// Tearing the out-link down under a transfer concludes it as `Failed(LinkClosed)`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_failed_link_closed_when_the_link_is_torn_down() {
    let pair = loopback_link_pair("outcome-link-closed").await;
    let destination = outcome_out_link_destination(&pair.connector.transport).await;

    let (_resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(
            &pair.link_id,
            random_payload(4 * MAX_EFFICIENT_SIZE + 1),
            None,
            true,
        )
        .await
        .expect("resource dispatched from the initiator side");
    pair.connector.transport.reset_out_link(&destination).await;

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::LinkClosed));
}

/// A request Resource over the listener's `max_request_size` is answered with a receiver
/// cancel (`RNS/Link.py:1036-1042`, `RNS/Resource.py:155-165`): the outcome resolves
/// `Rejected(PeerCancelled)`, the sender's event stream says the same, and nothing is
/// assembled on the listener.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_rejected_peer_cancelled_under_a_receiver_cap() {
    let pair =
        loopback_link_pair_with_request_limit("outcome-reject", Some(REJECT_REQUEST_LIMIT)).await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_request_resource_with_outcome(
            &pair.link_id,
            b"outcome-reject".to_vec(),
            random_payload(OVERSIZE_REQUEST_LEN),
            None,
            None,
        )
        .await
        .expect("oversize request resource dispatched");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Rejected(RejectReason::PeerCancelled));

    let rejected = await_outbound_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(
            rejected,
            ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
        ),
        "the event stream agrees with the outcome, got {rejected:?}"
    );
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;
}

/// `cancel_resource` on a transfer still in flight resolves `Cancelled`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_resolves_cancelled_on_cancel_resource() {
    let pair = loopback_link_pair("outcome-cancel").await;

    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(
            &pair.link_id,
            random_payload(4 * MAX_EFFICIENT_SIZE + 1),
            None,
            true,
        )
        .await
        .expect("resource dispatched from the initiator side");
    let cancelled = pair
        .connector
        .transport
        .cancel_resource(&pair.link_id, resource_hash)
        .await
        .expect("cancel");
    assert!(cancelled, "the transfer was still in flight when cancelled");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Cancelled);
}
