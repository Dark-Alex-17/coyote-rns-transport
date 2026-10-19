// Usage-side coverage of the `send_*_with_outcome` and `set_resource_accept_hook` surface,
// written from the README / CHANGELOG contract rather than from the implementation. It
// covers what the sibling `_outcome.rs` / `_accept_hook.rs` fixtures do not: dropping the
// outcome leaves the transfer running; the response-Resource variant resolves `Complete`
// under the requester's `max_response_size`, `Rejected(PeerCancelled)` over it and
// `Failed(RetriesExhausted)` when nobody expects the response (`RNS/Link.py:1044-1051`,
// `:1046-1047`); a stopped and dropped sender resolves `Failed(LinkClosed)`; the accept hook
// sees `None` before and `Some(identity)` after the peer identifies, is consulted exactly once
// for a split Resource it refuses (one receiver cancel, one `Rejected`), and never sees a
// response advertisement. Helpers come from the sibling `tests_parts` files.

/// A response the requester refuses at the advertisement: well over this cap.
const OUTCOME_PROBE_RESPONSE_CAP: usize = 1024;
/// A single-segment response body, over the cap above and under the one the within-cap
/// test grants.
const OUTCOME_PROBE_RESPONSE_LEN: usize = 8 * 1024;
/// Budget for the stop-and-drop verdict: the stop sweep budget plus margin.
const OUTCOME_PROBE_STOP_WAIT: Duration = Duration::from_secs(20);

/// Sends a request Resource from the connector under `request_id`, bounding the response at
/// `max_response_size`, and waits for the listener to assemble it so the response that
/// follows is solicited.
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_solicit_with_cap(
    pair: &LoopbackLinkPair,
    listener_events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
    max_response_size: Option<usize>,
) {
    let request_hash = pair
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &pair.link_id,
            request_id.to_vec(),
            random_payload(64),
            None,
            max_response_size,
        )
        .await
        .expect("request resource dispatched");
    let (complete, _segments) = await_complete_with_segments(listener_events, pair.link_id).await;
    assert_eq!(complete.hash, request_hash, "the listener assembled the request Resource");
}

/// Waits for the sender's terminal event for `resource_hash`.
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_await_terminal(
    events: &mut broadcast::Receiver<ResourceEvent>,
    resource_hash: Hash,
) -> ResourceEvent {
    await_resource_event(events, |event| {
        event.hash == resource_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. }
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
                    | ResourceEventKind::OutboundCancelled
            )
    })
    .await
}

/// Dropping the outcome cancels nothing; the receiver still assembles the split Resource
/// and the sender's events still carry `OutboundComplete`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_dropping_the_outcome_does_not_cancel_the_transfer() {
    let pair = loopback_link_pair("outcome-probe-drop").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut listener_events = pair.listener.transport.resource_events();

    let payload = random_payload(2 * MAX_EFFICIENT_SIZE + 17);
    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, payload.clone(), None, true)
        .await
        .expect("resource dispatched from the initiator side");
    drop(outcome);

    let (complete, segments_seen) =
        await_complete_with_segments(&mut listener_events, pair.link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments_seen, Some(3), "all three segments were delivered");
    assert!(complete_payload(complete).data == payload, "received bytes match the sent payload");

    let terminal = outcome_probe_await_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(terminal.kind, ResourceEventKind::OutboundComplete),
        "the sender's event stream still ends in OutboundComplete, got {:?}",
        terminal.kind
    );
}

/// A response Resource under the requester's `max_response_size` resolves `Complete` on
/// the responder's outcome and is assembled by the requester.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_response_resource_within_the_requester_cap_resolves_complete() {
    let pair = loopback_link_pair("outcome-probe-response-ok").await;
    let mut listener_events = pair.listener.transport.resource_events();
    let mut connector_events = pair.connector.transport.resource_events();
    let request_id = b"outcome-probe-response-ok";
    outcome_probe_solicit_with_cap(
        &pair,
        &mut listener_events,
        request_id,
        Some(OUTCOME_PROBE_RESPONSE_LEN * 4),
    )
    .await;

    let payload = random_payload(OUTCOME_PROBE_RESPONSE_LEN);
    let (response_hash, outcome) = pair
        .listener
        .transport
        .send_response_resource_with_outcome(
            &pair.link_id,
            request_id.to_vec(),
            payload.clone(),
            None,
            true,
        )
        .await
        .expect("response resource dispatched from the destination side");

    let (complete, _segments) =
        await_complete_with_segments(&mut connector_events, pair.link_id).await;
    assert_eq!(complete.hash, response_hash, "the requester assembled the response Resource");
    assert!(complete_payload(complete).data == payload, "received bytes match the response");
    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
}

/// A response Resource over the requester's `max_response_size` is refused with a receiver
/// cancel (`RNS/Link.py:1044-1051`): the responder's outcome resolves `Rejected(PeerCancelled)`
/// and the requester assembles nothing.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_response_resource_over_the_requester_cap_resolves_rejected() {
    let pair = loopback_link_pair("outcome-probe-response-cap").await;
    let mut listener_events = pair.listener.transport.resource_events();
    let mut connector_events = pair.connector.transport.resource_events();
    let request_id = b"outcome-probe-response-cap";
    outcome_probe_solicit_with_cap(
        &pair,
        &mut listener_events,
        request_id,
        Some(OUTCOME_PROBE_RESPONSE_CAP),
    )
    .await;

    let (response_hash, outcome) = pair
        .listener
        .transport
        .send_response_resource_with_outcome(
            &pair.link_id,
            request_id.to_vec(),
            random_payload(OUTCOME_PROBE_RESPONSE_LEN),
            None,
            true,
        )
        .await
        .expect("response resource dispatched from the destination side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Rejected(RejectReason::PeerCancelled));
    let terminal = outcome_probe_await_terminal(&mut listener_events, response_hash).await;
    assert!(
        matches!(
            terminal.kind,
            ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
        ),
        "the responder's event stream agrees, got {:?}",
        terminal.kind
    );
    assert_no_further_complete(&mut connector_events, pair.link_id).await;
}

/// README: a response Resource under a request id the requester never issued is dropped
/// without an answer (`RNS/Link.py:1046-1047`), so the responder's outcome ends in
/// `Failed(RetriesExhausted)` once the advertisement retries run out.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_unsolicited_response_resource_resolves_retries_exhausted() {
    let pair = loopback_link_pair("outcome-probe-unsolicited").await;
    let mut listener_events = pair.listener.transport.resource_events();
    let mut connector_events = pair.connector.transport.resource_events();

    let (response_hash, outcome) = pair
        .listener
        .transport
        .send_response_resource_with_outcome(
            &pair.link_id,
            b"outcome-probe-nobody-asked".to_vec(),
            random_payload(OUTCOME_PROBE_RESPONSE_LEN),
            None,
            true,
        )
        .await
        .expect("response resource dispatched from the destination side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::RetriesExhausted));
    let terminal = outcome_probe_await_terminal(&mut listener_events, response_hash).await;
    assert!(
        matches!(terminal.kind, ResourceEventKind::OutboundFailed),
        "the responder's event stream agrees, got {:?}",
        terminal.kind
    );
    assert_no_further_complete(&mut connector_events, pair.link_id).await;
}

/// README: "A transport dropped before a verdict resolves the outcome `Failed(LinkClosed)`
/// rather than leaving it pending." The listener is stalled so the transfer cannot finish
/// first; the connector is stopped and dropped under it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn outcome_probe_stopping_and_dropping_the_sender_resolves_failed_link_closed() {
    let LoopbackLinkPair { listener, connector, link_id } =
        loopback_link_pair("outcome-probe-stop-drop").await;
    let _stall = stall_transport_for(&listener.transport, Duration::from_secs(3)).await;

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

    connector.transport.stop().await;
    drop(connector);

    let verdict = timeout(OUTCOME_PROBE_STOP_WAIT, outcome)
        .await
        .expect("outcome resolved after the sender was stopped and dropped");
    assert_eq!(verdict, OutboundResult::Failed(OutboundFailure::LinkClosed));
}

/// The hook sees `None` for a peer that has not identified and `Some(identity)` once it
/// has (`RNS/Link.py:1065-1074` hands `ACCEPT_APP` the link, whose identity is the remote's
/// once identified).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_probe_sees_the_peer_identity_once_it_has_identified() {
    let mut pair = loopback_request_pair("accept-hook-probe-identity").await;
    let link_id = pair.handles.link_id;
    let mut listener_events = pair.handles.listener.transport.resource_events();

    let seen: Arc<std::sync::Mutex<Vec<Option<AddressHash>>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let hook = {
        let seen = seen.clone();
        Arc::new(
            move |_advertisement: &ResourceAdvertisement,
                  _link_id: &LinkId,
                  identity: Option<&Identity>| {
                seen.lock().expect("hook record").push(identity.map(|i| i.address_hash));
                true
            },
        )
    };
    pair.handles.listener.transport.set_resource_accept_hook(hook).await;

    let first = pair
        .handles
        .connector
        .transport
        .send_resource_with_outcome(&link_id, random_payload(4096), None, true)
        .await
        .expect("first resource dispatched");
    let (complete, _) = await_complete_with_segments(&mut listener_events, link_id).await;
    assert_eq!(complete.hash, first.0);
    assert_eq!(
        timeout(RESOURCE_TIMEOUT, first.1).await.expect("first outcome"),
        OutboundResult::Complete
    );

    let identity = PrivateIdentity::new_from_rand(OsRng);
    let identity_hash = identify_connector_as(&mut pair, &identity).await;

    let second = pair
        .handles
        .connector
        .transport
        .send_resource_with_outcome(&link_id, random_payload(4096), None, true)
        .await
        .expect("second resource dispatched");
    let (complete, _) = await_complete_with_segments(&mut listener_events, link_id).await;
    assert_eq!(complete.hash, second.0);
    assert_eq!(
        timeout(RESOURCE_TIMEOUT, second.1).await.expect("second outcome"),
        OutboundResult::Complete
    );

    let seen = seen.lock().expect("hook record").clone();
    assert_eq!(
        seen,
        vec![None, Some(identity_hash)],
        "the hook saw no identity before the peer identified and the peer's after"
    );
}

/// A refusing hook on a three-segment Resource is consulted once, draws exactly one receiver
/// cancel on the wire, and the sender gets exactly one terminal event; later segments are
/// never advertised.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_probe_refusing_a_split_resource_draws_one_cancel_and_one_rejected() {
    let mut pair = loopback_request_pair("accept-hook-probe-split").await;
    let link_id = pair.handles.link_id;
    let mut sender_events = pair.handles.connector.transport.resource_events();
    let mut listener_events = pair.handles.listener.transport.resource_events();

    let consulted = Arc::new(AtomicUsize::new(0));
    let hook = {
        let consulted = consulted.clone();
        Arc::new(move |_: &ResourceAdvertisement, _: &LinkId, _: Option<&Identity>| {
            consulted.fetch_add(1, Ordering::SeqCst);
            false
        })
    };
    pair.handles.listener.transport.set_resource_accept_hook(hook).await;

    let (resource_hash, outcome) = pair
        .handles
        .connector
        .transport
        .send_resource_with_outcome(
            &link_id,
            random_payload(2 * MAX_EFFICIENT_SIZE + 17),
            None,
            true,
        )
        .await
        .expect("split resource dispatched from the initiator side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Rejected(RejectReason::PeerCancelled));
    let terminal = outcome_probe_await_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(
            terminal.kind,
            ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
        ),
        "first terminal event is the rejection, got {:?}",
        terminal.kind
    );

    // Let any stray second advertisement or cancel land before counting.
    assert_no_further_complete(&mut listener_events, link_id).await;
    assert_eq!(
        consulted.load(Ordering::SeqCst),
        1,
        "the hook ran once: no later segment was offered"
    );
    let cancels =
        count_link_packets(&mut pair.connector_tap, link_id, PacketContext::ResourceReceiverCancel);
    assert_eq!(cancels, 1, "exactly one receiver cancel reached the sender");
    let deadline = Instant::now() + RESOURCE_SETTLE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match timeout(remaining, sender_events.recv()).await {
            Ok(Ok(event)) => assert!(
                !(event.hash == resource_hash
                    && matches!(
                        event.kind,
                        ResourceEventKind::OutboundRejected { .. }
                            | ResourceEventKind::OutboundComplete
                            | ResourceEventKind::OutboundFailed
                            | ResourceEventKind::OutboundCancelled
                    )),
                "a second terminal event arrived for the refused resource: {:?}",
                event.kind
            ),
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        }
    }
}

/// A response advertisement belongs to the `max_response_size` branch
/// (`RNS/Link.py:1044-1051`), so a refusing hook on the requester never sees it and the
/// response Resource completes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_probe_is_not_consulted_for_a_response_advertisement() {
    let pair = loopback_link_pair("accept-hook-probe-response").await;
    let mut listener_events = pair.listener.transport.resource_events();
    let mut connector_events = pair.connector.transport.resource_events();

    let consulted = Arc::new(AtomicUsize::new(0));
    let hook = {
        let consulted = consulted.clone();
        Arc::new(move |_: &ResourceAdvertisement, _: &LinkId, _: Option<&Identity>| {
            consulted.fetch_add(1, Ordering::SeqCst);
            false
        })
    };
    pair.connector.transport.set_resource_accept_hook(hook).await;

    let request_id = b"accept-hook-probe-response";
    outcome_probe_solicit_with_cap(&pair, &mut listener_events, request_id, None).await;
    let payload = random_payload(OUTCOME_PROBE_RESPONSE_LEN);
    let (response_hash, outcome) = pair
        .listener
        .transport
        .send_response_resource_with_outcome(
            &pair.link_id,
            request_id.to_vec(),
            payload.clone(),
            None,
            true,
        )
        .await
        .expect("response resource dispatched from the destination side");

    let (complete, _segments) =
        await_complete_with_segments(&mut connector_events, pair.link_id).await;
    assert_eq!(complete.hash, response_hash, "the requester assembled the response Resource");
    assert!(complete_payload(complete).data == payload, "received bytes match the response");
    assert_eq!(
        timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved"),
        OutboundResult::Complete
    );
    assert_eq!(consulted.load(Ordering::SeqCst), 0, "the hook never saw the response");
}
