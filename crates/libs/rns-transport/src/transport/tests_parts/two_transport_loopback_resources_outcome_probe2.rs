// Further usage-side coverage of the `send_*_with_outcome` and `set_resource_accept_hook`
// surface, written from the README / CHANGELOG contract rather than from the implementation:
// a hook that applies a size policy read off the advertisement sorts two sends in flight on
// one link into `Complete` and `Rejected(PeerCancelled)` with independent outcomes; two split
// Resources in flight at once each resolve `Complete` under their own hash; `Transport::stop`
// concludes every outbound transfer on a link it locks within the stop budget, not just one;
// and a sender dropped without `stop` still resolves `Failed(LinkClosed)`. Helpers come from
// the sibling `tests_parts` files included into this module.

/// The size policy the hook applies: advertisements over this many bytes are refused.
const OUTCOME_PROBE2_HOOK_LIMIT: u64 = 8 * 1024;
/// A single-segment payload under the policy limit.
const OUTCOME_PROBE2_SMALL_LEN: usize = 1024;
/// A single-segment payload over the policy limit and under `MAX_EFFICIENT_SIZE`.
const OUTCOME_PROBE2_LARGE_LEN: usize = 64 * 1024;
/// Keeps the listener from answering before the connector is dropped.
const OUTCOME_PROBE2_DROP_STALL: Duration = Duration::from_secs(3);
/// Budget for the drop verdict: worker shutdown on cancellation plus margin.
const OUTCOME_PROBE2_DROP_WAIT: Duration = Duration::from_secs(20);

/// Collects the sender's terminal events for every hash in `expected`, in arrival order.
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_await_terminals(
    events: &mut broadcast::Receiver<ResourceEvent>,
    expected: &[Hash],
) -> Vec<ResourceEvent> {
    let mut terminals = Vec::new();
    while terminals.len() < expected.len() {
        let event = await_resource_event(events, |event| {
            expected.contains(&event.hash)
                && matches!(
                    event.kind,
                    ResourceEventKind::OutboundRejected { .. }
                        | ResourceEventKind::OutboundComplete
                        | ResourceEventKind::OutboundFailed
                        | ResourceEventKind::OutboundCancelled
                )
        })
        .await;
        assert!(
            !terminals.iter().any(|seen: &ResourceEvent| seen.hash == event.hash),
            "a second terminal event arrived for {}",
            event.hash
        );
        terminals.push(event);
    }
    terminals
}

/// Collects the receiver's `Complete` events on `link_id` until every hash in `expected` has
/// arrived, failing on any inbound failure on the way.
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_await_completes(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    expected: &[Hash],
) -> Vec<(Hash, ResourceComplete)> {
    let mut completes = Vec::new();
    while completes.len() < expected.len() {
        let (complete, _segments) = await_complete_with_segments(events, link_id).await;
        assert!(
            expected.contains(&complete.hash),
            "the receiver completed a hash nobody sent: {}",
            complete.hash
        );
        assert!(
            !completes.iter().any(|(hash, _): &(Hash, ResourceComplete)| *hash == complete.hash),
            "a second Complete arrived for {}",
            complete.hash
        );
        completes.push((complete.hash, complete_payload(complete)));
    }
    completes
}

/// Two plain Resources dispatched back to back before either verdict is awaited, with a
/// hook on the receiver that reads the advertisement's `data_size`: the one under the policy
/// limit resolves `Complete` and is assembled, the one over it resolves
/// `Rejected(PeerCancelled)` and is not, and the hook saw each advertisement under the hash
/// the sender was handed.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_size_policy_hook_sorts_two_sends_in_flight_on_one_link() {
    let pair = loopback_link_pair("outcome-probe2-size-policy").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let seen = Arc::new(StdMutex::new(Vec::<(Hash, u64)>::new()));
    let hook = {
        let seen = seen.clone();
        Arc::new(
            move |advertisement: &ResourceAdvertisement,
                  _link_id: &LinkId,
                  _identity: Option<&Identity>| {
                seen.lock()
                    .expect("hook record")
                    .push((advertisement.hash, advertisement.data_size));
                advertisement.data_size <= OUTCOME_PROBE2_HOOK_LIMIT
            },
        )
    };
    pair.listener.transport.set_resource_accept_hook(hook).await;

    let small = random_payload(OUTCOME_PROBE2_SMALL_LEN);
    let large = random_payload(OUTCOME_PROBE2_LARGE_LEN);
    let (small_hash, small_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, small.clone(), None, true)
        .await
        .expect("small resource dispatched");
    let (large_hash, large_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, large.clone(), None, true)
        .await
        .expect("large resource dispatched");
    assert_ne!(small_hash, large_hash, "two payloads hash differently");

    let (small_verdict, large_verdict) = tokio::join!(
        timeout(RESOURCE_TIMEOUT, small_outcome),
        timeout(RESOURCE_TIMEOUT, large_outcome)
    );
    assert_eq!(small_verdict.expect("small outcome resolved"), OutboundResult::Complete);
    assert_eq!(
        large_verdict.expect("large outcome resolved"),
        OutboundResult::Rejected(RejectReason::PeerCancelled)
    );

    let completes =
        outcome_probe2_await_completes(&mut receiver_events, pair.link_id, &[small_hash]).await;
    assert_eq!(completes[0].0, small_hash, "the receiver assembled the admitted resource");
    assert!(completes[0].1.data == small, "received bytes match the admitted payload");
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;

    let terminals =
        outcome_probe2_await_terminals(&mut sender_events, &[small_hash, large_hash]).await;
    for terminal in &terminals {
        if terminal.hash == small_hash {
            assert!(
                matches!(terminal.kind, ResourceEventKind::OutboundComplete),
                "events agree with the small outcome, got {:?}",
                terminal.kind
            );
        } else {
            assert!(
                matches!(
                    terminal.kind,
                    ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
                ),
                "events agree with the large outcome, got {:?}",
                terminal.kind
            );
        }
    }

    let seen = seen.lock().expect("hook record").clone();
    assert_eq!(seen.len(), 2, "the hook ran once per advertisement, saw {seen:?}");
    assert!(
        seen.contains(&(small_hash, OUTCOME_PROBE2_SMALL_LEN as u64)),
        "the hook saw the small advertisement under its hash and size, saw {seen:?}"
    );
    assert!(
        seen.contains(&(large_hash, OUTCOME_PROBE2_LARGE_LEN as u64)),
        "the hook saw the large advertisement under its hash and size, saw {seen:?}"
    );
}

/// Two split Resources dispatched before either verdict is awaited both resolve `Complete`,
/// each under its own hash, and the receiver assembles both payloads intact.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_two_split_sends_in_flight_resolve_independently() {
    let pair = loopback_link_pair("outcome-probe2-two-split").await;
    let mut receiver_events = pair.listener.transport.resource_events();

    let first = random_payload(2 * MAX_EFFICIENT_SIZE + 17);
    let second = random_payload(2 * MAX_EFFICIENT_SIZE + 33);
    let (first_hash, first_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, first.clone(), None, true)
        .await
        .expect("first split resource dispatched");
    let (second_hash, second_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, second.clone(), None, true)
        .await
        .expect("second split resource dispatched");
    assert_ne!(first_hash, second_hash, "two payloads hash differently");

    let (first_verdict, second_verdict) = tokio::join!(
        timeout(RESOURCE_TIMEOUT, first_outcome),
        timeout(RESOURCE_TIMEOUT, second_outcome)
    );
    assert_eq!(first_verdict.expect("first outcome resolved"), OutboundResult::Complete);
    assert_eq!(second_verdict.expect("second outcome resolved"), OutboundResult::Complete);

    let completes = outcome_probe2_await_completes(
        &mut receiver_events,
        pair.link_id,
        &[first_hash, second_hash],
    )
    .await;
    for (hash, complete) in &completes {
        let expected = if *hash == first_hash { &first } else { &second };
        assert!(complete.data == *expected, "received bytes for {hash} match the sent payload");
    }
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;
}

/// `Transport::stop` with two outbound transfers in flight concludes both `Failed(LinkClosed)`
/// inside the stop budget, so neither outcome is left pending.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_stop_concludes_every_outbound_transfer_in_flight() {
    let pair = loopback_link_pair("outcome-probe2-stop-two").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let stall = stall_transport_for(&pair.listener.transport, OUTCOME_STOP_STALL).await;

    let (first_hash, first_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(
            &pair.link_id,
            random_payload(2 * MAX_EFFICIENT_SIZE),
            None,
            true,
        )
        .await
        .expect("first resource dispatched");
    let (second_hash, second_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(
            &pair.link_id,
            random_payload(2 * MAX_EFFICIENT_SIZE + 1),
            None,
            true,
        )
        .await
        .expect("second resource dispatched");

    pair.connector.transport.stop().await;

    let (first_verdict, second_verdict) = tokio::join!(
        timeout(STOP_BUDGET + OUTCOME_STOP_SLACK, first_outcome),
        timeout(STOP_BUDGET + OUTCOME_STOP_SLACK, second_outcome)
    );
    assert_eq!(
        first_verdict.expect("the stop sweep resolved the first outcome"),
        OutboundResult::Failed(OutboundFailure::LinkClosed)
    );
    assert_eq!(
        second_verdict.expect("the stop sweep resolved the second outcome"),
        OutboundResult::Failed(OutboundFailure::LinkClosed)
    );

    let terminals =
        outcome_probe2_await_terminals(&mut sender_events, &[first_hash, second_hash]).await;
    for terminal in &terminals {
        assert!(
            matches!(terminal.kind, ResourceEventKind::OutboundFailed),
            "the event stream agrees with the outcome for {}, got {:?}",
            terminal.hash,
            terminal.kind
        );
    }
    stall.await.expect("stall task finished");
}

/// README: "A transport dropped before a verdict ... resolves the outcome `Failed(LinkClosed)`
/// rather than leaving it pending." The sender is dropped without `stop`; the listener is
/// stalled so the transfer cannot finish first.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe2_dropping_the_sender_without_stop_resolves_failed_link_closed() {
    let LoopbackLinkPair { listener, connector, link_id } =
        loopback_link_pair("outcome-probe2-drop").await;
    let stall = stall_transport_for(&listener.transport, OUTCOME_PROBE2_DROP_STALL).await;

    let (_resource_hash, outcome) = connector
        .transport
        .send_resource_with_outcome(
            &link_id,
            random_payload(4 * MAX_EFFICIENT_SIZE + 1),
            None,
            true,
        )
        .await
        .expect("resource dispatched from the initiator side");

    let dropped_at = Instant::now();
    drop(connector);

    let verdict = timeout(OUTCOME_PROBE2_DROP_WAIT, outcome)
        .await
        .expect("outcome resolved after the sender was dropped without stop");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::LinkClosed));
    assert!(
        dropped_at.elapsed() < OUTCOME_PROBE2_DROP_WAIT,
        "verdict took {:?} after the drop",
        dropped_at.elapsed()
    );
    stall.await.expect("stall task finished");
}
