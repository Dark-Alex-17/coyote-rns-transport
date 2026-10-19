// Loopback coverage for `Transport::set_resource_accept_hook`: a hook refusing a plain
// advertisement draws the same receiver cancel the size caps do (`RNS/Link.py:1065-1074`,
// `RNS/Resource.py:155-165`), the link survives it, and a replacement hook admits the next
// transfer; a hook that panics takes neither the advertisement's answer nor the transport
// down. Helpers come from the sibling `tests_parts` files included into this module.

use std::sync::atomic::AtomicBool;

/// How long the sender's event stream is watched for a receiver cancel that must not come.
const HOOK_PANIC_SETTLE: Duration = Duration::from_secs(1);

/// A hook returning `false` on the receiving side: the sender's outcome and event stream
/// both read `Rejected(PeerCancelled)`, the hook saw the link the transfer arrived on, and
/// the listener assembled nothing. A hook returning `true` installed in its place then
/// lets a transfer complete over the same link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_refusal_rejects_the_transfer_and_the_link_survives() {
    let pair = loopback_link_pair("accept-hook").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let consulted = Arc::new(AtomicUsize::new(0));
    let link_matched = Arc::new(AtomicBool::new(false));
    let hook = {
        let consulted = consulted.clone();
        let link_matched = link_matched.clone();
        let expected_link = pair.link_id;
        Arc::new(
            move |_advertisement: &ResourceAdvertisement,
                  link_id: &LinkId,
                  _identity: Option<&Identity>| {
                consulted.fetch_add(1, Ordering::SeqCst);
                link_matched.store(*link_id == expected_link, Ordering::SeqCst);
                false
            },
        )
    };
    pair.listener.transport.set_resource_accept_hook(hook).await;

    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, random_payload(4096), None, true)
        .await
        .expect("resource dispatched from the initiator side");

    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Rejected(RejectReason::PeerCancelled));
    assert_eq!(consulted.load(Ordering::SeqCst), 1, "the hook ran once for one advertisement");
    assert!(link_matched.load(Ordering::SeqCst), "the hook saw the transfer's link");

    let rejected = await_resource_event(&mut sender_events, |event| {
        event.hash == resource_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. }
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(
            rejected.kind,
            ResourceEventKind::OutboundRejected { reason: RejectReason::PeerCancelled }
        ),
        "the event stream agrees with the outcome, got {:?}",
        rejected.kind
    );
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;

    pair.listener
        .transport
        .set_resource_accept_hook(Arc::new(|_advertisement, _link_id, _identity| true))
        .await;
    let payload = random_payload(4096);
    let (resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, payload.clone(), None, true)
        .await
        .expect("second resource dispatched over the surviving link");

    let (complete, _segments) =
        await_complete_with_segments(&mut receiver_events, pair.link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert!(complete_payload(complete).data == payload, "received bytes match the sent payload");
    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
}

/// A request advertisement belongs to the `max_request_size` branch
/// (`RNS/Link.py:1036-1042`), so a refusing hook never sees it and the request Resource
/// completes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_is_not_consulted_for_a_request_advertisement() {
    let pair = loopback_link_pair("accept-hook-request").await;
    let mut receiver_events = pair.listener.transport.resource_events();

    let consulted = Arc::new(AtomicUsize::new(0));
    let hook = {
        let consulted = consulted.clone();
        Arc::new(move |_: &ResourceAdvertisement, _: &LinkId, _: Option<&Identity>| {
            consulted.fetch_add(1, Ordering::SeqCst);
            false
        })
    };
    pair.listener.transport.set_resource_accept_hook(hook).await;

    let resource_hash =
        solicit_response(&pair.connector.transport, &pair.link_id, b"accept-hook-request").await;

    let (complete, _segments) =
        await_complete_with_segments(&mut receiver_events, pair.link_id).await;
    assert_eq!(complete.hash, resource_hash, "the request Resource was assembled");
    assert_eq!(consulted.load(Ordering::SeqCst), 0, "the hook never saw the request");
}

/// A hook that panics is contained: the advertisement is dropped unanswered, so the sender
/// sees no receiver cancel (`RNS/Link.py:1073-1074` logs the exception and neither accepts
/// nor rejects), and the listener's workers survive to complete the next transfer once a
/// well-behaved hook replaces it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn accept_hook_panic_drops_the_advertisement_and_the_transport_survives() {
    let pair = loopback_link_pair("accept-hook-panic").await;
    let mut sender_events = pair.connector.transport.resource_events();

    pair.listener
        .transport
        .set_resource_accept_hook(Arc::new(|_advertisement, _link_id, _identity| {
            panic!("accept hook under test")
        }))
        .await;

    let (resource_hash, _outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, random_payload(4096), None, true)
        .await
        .expect("resource dispatched from the initiator side");

    let deadline = Instant::now() + HOOK_PANIC_SETTLE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let Ok(Ok(event)) = timeout(remaining, sender_events.recv()).await else {
            break;
        };
        assert!(
            !(event.hash == resource_hash
                && matches!(event.kind, ResourceEventKind::OutboundRejected { .. })),
            "a panicking hook must not answer with a receiver cancel"
        );
    }

    pair.listener
        .transport
        .set_resource_accept_hook(Arc::new(|_advertisement, _link_id, _identity| true))
        .await;
    let (_resource_hash, outcome) = pair
        .connector
        .transport
        .send_resource_with_outcome(&pair.link_id, random_payload(4096), None, true)
        .await
        .expect("second resource dispatched over the surviving link");
    let verdict = timeout(RESOURCE_TIMEOUT, outcome).await.expect("outcome resolved");
    assert_eq!(verdict, OutboundResult::Complete);
}
