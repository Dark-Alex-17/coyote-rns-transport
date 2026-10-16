use crate::resource::{ResourceComplete, MAX_EFFICIENT_SIZE};

/// A 4 MiB transfer over loopback is quick, but CI hosts are not; the budget covers the
/// whole transfer, not one event.
const RESOURCE_TIMEOUT: Duration = Duration::from_secs(90);
const RESOURCE_SETTLE: Duration = Duration::from_millis(500);

/// Waits for the first resource event matching `predicate`. Progress events arrive far
/// faster than this receiver drains them, so lag is expected and skipped.
#[deny(clippy::await_holding_invalid_type)]
async fn await_resource_event(
    events: &mut broadcast::Receiver<ResourceEvent>,
    mut predicate: impl FnMut(&ResourceEvent) -> bool,
) -> ResourceEvent {
    timeout(RESOURCE_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if predicate(&event) => return event,
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("resource event channel closed"),
            }
        }
    })
    .await
    .expect("timed out waiting for the resource event")
}

/// Drains the receiver until `Complete` for `link_id` arrives, returning it together with
/// the highest `total_segments` seen on the way.
#[deny(clippy::await_holding_invalid_type)]
async fn await_complete_with_segments(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
) -> (ResourceEvent, Option<u32>) {
    let mut segments_seen = None;
    let complete = await_resource_event(events, |event| {
        if event.link_id != link_id {
            return false;
        }
        match &event.kind {
            ResourceEventKind::SegmentComplete(segment) => {
                segments_seen = segments_seen.max(Some(segment.total_segments));
                false
            }
            ResourceEventKind::Complete(_) => true,
            ResourceEventKind::InboundFailed(failure) => {
                panic!("inbound resource failed on link {link_id}: {failure:?}")
            }
            _ => false,
        }
    })
    .await;
    (complete, segments_seen)
}

/// A second `Complete` for the same link would mean a segment was delivered twice or the
/// assembly was finished more than once; neither surfaces in the first event.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_no_further_complete(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
) {
    let deadline = Instant::now() + RESOURCE_SETTLE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        match timeout(remaining, events.recv()).await {
            Ok(Ok(event)) => {
                assert!(
                    !(event.link_id == link_id
                        && matches!(event.kind, ResourceEventKind::Complete(_))),
                    "a second Complete arrived for link {link_id}"
                );
            }
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return,
        }
    }
}

fn complete_payload(event: ResourceEvent) -> ResourceComplete {
    match event.kind {
        ResourceEventKind::Complete(complete) => complete,
        other => panic!("expected a Complete event, got {other:?}"),
    }
}

/// Sends a small request Resource under `request_id` from `requester` and returns its hash,
/// so that a response the peer sends under that id is solicited: a response advertisement
/// whose `q` matches no pending request is dropped (`RNS/Link.py:1046-1047`). The bytes are
/// not a request frame, so nothing answers the request itself.
#[deny(clippy::await_holding_invalid_type)]
async fn solicit_response(requester: &Transport, link_id: &AddressHash, request_id: &[u8]) -> Hash {
    requester
        .send_request_resource(link_id, request_id.to_vec(), random_payload(64), None)
        .await
        .expect("request resource dispatched")
}

/// Sends a response Resource of `payload_len` random bytes from the listener (the link's
/// destination side) to the connector over a broadcast-off loopback pair and checks both
/// ends see it finish. Random bytes keep each segment's compressed transfer size and part
/// count realistic; the segment count itself is fixed by the raw size.
#[deny(clippy::await_holding_invalid_type)]
async fn response_resource_round_trip(payload_len: usize, expected_segments: u32) {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();

    let (_destination, desc) =
        register_in_destination(&listener.transport, "loopback", "resource").await;
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;
    let (_link, link_id) = open_link(&connector.transport, learned).await;
    let activated = await_event(&mut in_events, |event| {
        matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");

    let mut sender_events = listener.transport.resource_events();
    let mut receiver_events = connector.transport.resource_events();

    let mut payload = vec![0u8; payload_len];
    OsRng.fill_bytes(&mut payload);
    let request_id = b"loopback-response".to_vec();
    solicit_response(&connector.transport, &link_id, &request_id).await;
    let resource_hash = listener
        .transport
        .send_response_resource(&link_id, request_id.clone(), payload.clone(), None)
        .await
        .expect("response resource dispatched");

    let (complete, segments_seen) =
        await_complete_with_segments(&mut receiver_events, link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    if expected_segments > 1 {
        assert!(
            segments_seen.is_some(),
            "no SegmentComplete observed on the receiver (broadcast lag?)"
        );
        assert_eq!(segments_seen, Some(expected_segments), "segment count on the receiver");
    } else {
        assert_eq!(segments_seen, None, "a single-segment transfer emits no SegmentComplete");
    }
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert!(!complete.is_request, "not advertised as a request");
    assert_eq!(complete.request_id.as_deref(), Some(request_id.as_slice()));
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_single_segment() {
    response_resource_round_trip(300 * 1024, 1).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_two_segments() {
    response_resource_round_trip(MAX_EFFICIENT_SIZE + 64 * 1024, 2).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_five_segments() {
    response_resource_round_trip(4 * 1024 * 1024 + 1, 5).await;
}
