// Third usage-side pass over the `send_*_with_outcome`, `set_resource_accept_hook` and
// `Transport::stop` contract, written from the README and CHANGELOG wording rather than
// from the implementation: an accepting hook is consulted once per segment advertisement
// of a split Resource and sees the sender's hash as `original_hash` on each; a panicking
// hook that is never replaced leaves the advertisement unanswered, so the sender
// resolves `Failed(RetriesExhausted)` once its advertisement retries run out and the
// receiver assembles nothing; and `stop()` skips a link whose lock it cannot take within the
// stop budget, leaving that transfer's outcome pending until the transport is dropped, at
// which point it resolves `Failed(LinkClosed)`. Helpers come from the sibling `tests_parts`
// files included into this module.

/// Keeps the listener from answering until the connector's link lock is held.
const OUTCOME_PROBE3_HELD_STALL: Duration = Duration::from_secs(3);
/// Scheduling slack on top of the stop budget `stop` may return late by.
const OUTCOME_PROBE3_STOP_SLACK: Duration = Duration::from_secs(3);
/// How long a still-pending outcome is polled before the test is satisfied it stays so.
const OUTCOME_PROBE3_PENDING_WINDOW: Duration = Duration::from_millis(500);

/// README: "A split resource consults the hook once per segment advertisement". A hook
/// returning `true` on a three-segment Resource runs three times, each advertisement names
/// the sender's hash as its `original_hash`, the three segment hashes differ, and the
/// transfer resolves `Complete` with the payload intact.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe3_accepting_hook_runs_once_per_segment_advertisement() {
    let pair = loopback_link_pair("outcome-probe3-per-segment").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let seen = Arc::new(StdMutex::new(Vec::<(Hash, Hash, u32, u32)>::new()));
    let hook = {
        let seen = seen.clone();
        Arc::new(
            move |advertisement: &ResourceAdvertisement,
                  _link_id: &LinkId,
                  _identity: Option<&Identity>| {
                seen.lock().expect("hook record").push((
                    advertisement.original_hash,
                    advertisement.hash,
                    advertisement.segment_index,
                    advertisement.total_segments,
                ));
                true
            },
        )
    };
    pair.listener.transport.set_resource_accept_hook(hook).await;

    let payload = random_payload(2 * MAX_EFFICIENT_SIZE + 17);
    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, payload.clone(), None, true)
        .await
        .expect("split resource dispatched from the initiator side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
    let (complete, segments) =
        await_complete_with_segments(&mut receiver_events, pair.link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments, Some(3), "three segments crossed the link");
    assert!(complete_payload(complete).data == payload, "received bytes match the payload");
    let terminal = outcome_probe_await_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(terminal.kind, ResourceEventKind::OutboundComplete),
        "the event stream agrees with the outcome, got {:?}",
        terminal.kind
    );

    let seen = seen.lock().expect("hook record").clone();
    assert_eq!(seen.len(), 3, "the hook ran once per segment advertisement: {seen:?}");
    for (original, _segment, index, total) in &seen {
        assert_eq!(*original, resource_hash, "every advertisement names the sender's hash");
        assert_eq!(*total, 3, "every advertisement carries the segment total");
        assert!(*index >= 1 && *index <= 3, "segment index in range, got {index}");
    }
    let mut indices: Vec<u32> = seen.iter().map(|(_, _, index, _)| *index).collect();
    indices.sort_unstable();
    assert_eq!(indices, vec![1, 2, 3], "each segment was offered exactly once");
    let mut segment_hashes: Vec<Hash> = seen.iter().map(|(_, segment, _, _)| *segment).collect();
    segment_hashes.sort_unstable_by(|a, b| a.as_slice().cmp(b.as_slice()));
    segment_hashes.dedup();
    assert_eq!(segment_hashes.len(), 3, "the three segment advertisements hash differently");
}

/// README: a panicking hook drops the advertisement "neither accepted nor refused". With no
/// well-behaved replacement the sender's advertisement goes unanswered through every retry
/// and, like any unanswered advertisement, ends `Failed(RetriesExhausted)` on both the
/// outcome and the event stream; the receiver assembles nothing and its workers are still
/// there to admit a transfer once a replacement hook is installed. The retries never reach
/// the hook: the sender re-sends the prepared advertisement packet, so every retry carries
/// the first packet's hash and the receiver's duplicate filter drops it before admission
/// (Python builds a fresh packet per retry, `RNS/Resource.py:597-598`, and so re-runs its
/// callback). The test pins the contract the README states, not the count of
/// consultations.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe3_panicking_hook_left_in_place_resolves_retries_exhausted() {
    let pair = loopback_link_pair("outcome-probe3-panic-exhausts").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let consulted = Arc::new(AtomicUsize::new(0));
    let hook = {
        let consulted = consulted.clone();
        Arc::new(move |_: &ResourceAdvertisement, _: &LinkId, _: Option<&Identity>| {
            consulted.fetch_add(1, Ordering::SeqCst);
            panic!("accept hook under test")
        })
    };
    pair.listener.transport.set_resource_accept_hook(hook).await;

    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, random_payload(4096), None, true)
        .await
        .expect("resource dispatched from the initiator side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::RetriesExhausted));
    let terminal = outcome_probe_await_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(terminal.kind, ResourceEventKind::OutboundFailed),
        "the event stream agrees with the outcome, got {:?}",
        terminal.kind
    );
    assert!(
        consulted.load(Ordering::SeqCst) >= 1,
        "the advertisement reached the hook before it panicked"
    );
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;

    pair.listener
        .transport
        .set_resource_accept_hook(Arc::new(|_advertisement, _link_id, _identity| true))
        .await;
    let payload = random_payload(4096);
    let (second_hash, second_outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, payload.clone(), None, true)
        .await
        .expect("second resource dispatched over the surviving link");
    let verdict = timeout(RESOURCE_TIMEOUT, second_outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
    let (complete, _segments) =
        await_complete_with_segments(&mut receiver_events, pair.link_id).await;
    assert_eq!(complete.hash, second_hash, "the receiver assembled the second resource");
    assert!(complete_payload(complete).data == payload, "received bytes match the payload");
}

/// README: `stop()` concludes outbound transfers "on a link its sweep locks within the 5 s
/// stop budget, and a transfer on a link it had to skip keeps its outcome until the
/// transport is dropped". The connector's only link is held by another thread across
/// `stop()`: `stop` still returns inside the budget, the outcome is still pending once it
/// has, and dropping the transport resolves it `Failed(LinkClosed)`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe3_stop_skips_a_held_link_and_the_outcome_concludes_on_the_drop() {
    let pair = loopback_request_pair("outcome-probe3-held-link").await;
    let link_id = pair.handles.link_id;
    let stall =
        stall_transport_for(&pair.handles.listener.transport, OUTCOME_PROBE3_HELD_STALL).await;

    let (_resource_hash, mut outcome) = pair
        .handles
        .connector
        .transport
        .send_resource_with_outcome(&link_id, random_payload(2 * MAX_EFFICIENT_SIZE), None, true)
        .await
        .expect("split resource dispatched from the initiator side");

    let held = hold_link(pair.handles.link.clone());
    held.locked.await.expect("the holder takes the lock");

    let started = std::time::Instant::now();
    pair.handles.connector.transport.stop().await;
    let elapsed = started.elapsed();
    assert!(
        elapsed < STOP_BUDGET + OUTCOME_PROBE3_STOP_SLACK,
        "stop overran its budget: {elapsed:?}"
    );
    assert!(
        elapsed >= STOP_BUDGET,
        "stop returned before the held link's wait ran out: {elapsed:?}"
    );
    assert!(
        timeout(OUTCOME_PROBE3_PENDING_WINDOW, &mut outcome).await.is_err(),
        "the sweep skipped the held link, so its outcome stays pending after stop"
    );

    held.release.send(()).expect("the holder waits for the release");
    held.thread.join().expect("the holder releases the lock");
    stall.await.expect("stall task finished");
    drop(pair);

    let verdict = timeout(LINK_DROP_TIMEOUT, outcome)
        .await
        .expect("the outcome concludes once the transport is dropped");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::LinkClosed));
}
