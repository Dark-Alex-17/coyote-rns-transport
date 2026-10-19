// Loopback coverage for `Transport::stop` against an outbound Resource still in flight: the
// stop sweep concludes it `Failed(LinkClosed)`, so an outcome the caller still holds resolves
// inside `stop` instead of waiting on a transport whose workers no longer run. Helpers come
// from the sibling `tests_parts` files.

/// Long enough to keep the listener from answering the advertisement before `stop` detaches
/// the connector's interfaces; after that nothing the listener does can reach the sender.
const OUTCOME_STOP_STALL: Duration = Duration::from_secs(2);

/// Slack on top of the stop budget. `stop` has returned by the time the outcome is awaited,
/// so the wait only bounds a regression that leaves it pending.
const OUTCOME_STOP_SLACK: Duration = Duration::from_secs(5);

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_stop_concludes_an_outbound_resource_link_closed() {
    let pair = loopback_link_pair("outcome-stop").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let stall = stall_transport_for(&pair.listener.transport, OUTCOME_STOP_STALL).await;

    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(
            &pair.link_id,
            random_payload(2 * MAX_EFFICIENT_SIZE),
            None,
            true,
        )
        .await
        .expect("resource dispatched from the initiator side");

    pair.connector.transport.stop().await;

    let verdict = timeout(STOP_BUDGET + OUTCOME_STOP_SLACK, outcome)
        .await
        .expect("the stop sweep resolved the outcome");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::LinkClosed));
    let failed = await_resource_event(&mut sender_events, |event| {
        event.hash == resource_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundFailed
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundRejected { .. }
            )
    })
    .await;
    assert!(
        matches!(failed.kind, ResourceEventKind::OutboundFailed),
        "the event stream agrees with the outcome, got {:?}",
        failed.kind
    );
    stall.await.expect("stall task finished");
}
