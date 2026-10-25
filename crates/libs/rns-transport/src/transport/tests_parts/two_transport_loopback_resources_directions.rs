// Loopback coverage for later Resource segments in the directions the sibling file does not
// take: the initiator (out-link) side sending a split Resource, a split request Resource, and
// two split Resources back to back on one link. Helpers come from the sibling `tests_parts`
// files included into this module.

/// A connected broadcast-off loopback pair with one activated link between them.
struct LoopbackLinkPair {
    listener: LoopbackListener,
    connector: LoopbackConnector,
    link_id: AddressHash,
}

#[deny(clippy::await_holding_invalid_type)]
async fn loopback_link_pair(aspect: &str) -> LoopbackLinkPair {
    loopback_link_pair_with_request_limit(aspect, None).await
}

/// Like `loopback_link_pair`, with an inbound request-size limit armed on the listener's
/// destination before the link opens when `request_limit` is set.
#[deny(clippy::await_holding_invalid_type)]
async fn loopback_link_pair_with_request_limit(
    aspect: &str,
    request_limit: Option<usize>,
) -> LoopbackLinkPair {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();

    let (destination, desc) =
        register_in_destination(&listener.transport, "loopback", aspect).await;
    if let Some(limit) = request_limit {
        destination
            .lock()
            .await
            .set_max_request_size(limit)
            .expect("request-size limit armed");
    }
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;
    let (_link, link_id) = open_link(&connector.transport, learned).await;
    let activated = await_event(&mut in_events, |event| {
        matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
    LoopbackLinkPair {
        listener,
        connector,
        link_id,
    }
}

fn random_payload(len: usize) -> Vec<u8> {
    let mut payload = vec![0u8; len];
    OsRng.fill_bytes(&mut payload);
    payload
}

/// Checks one receiver-side `Complete` plus the sender's clean terminal event for a
/// transfer that was dispatched as `resource_hash`.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_transfer_completes(
    sender_events: &mut broadcast::Receiver<ResourceEvent>,
    receiver_events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    resource_hash: Hash,
    payload: &[u8],
    expected_segments: u32,
) -> ResourceComplete {
    let (complete, segments_seen) =
        await_complete_with_segments(receiver_events, link_id).await;
    assert_eq!(complete.hash, resource_hash, "receiver reports the sender's resource hash");
    assert_eq!(segments_seen, Some(expected_segments), "segment count on the receiver");
    let complete = complete_payload(complete);
    assert_eq!(complete.data.len(), payload.len(), "received length");
    assert!(complete.data == payload, "received bytes match the sent payload");

    let outbound = await_outbound_terminal(sender_events, resource_hash).await;
    assert!(
        matches!(outbound, ResourceEventKind::OutboundComplete),
        "sender should finish cleanly, got {outbound:?}"
    );
    complete
}

/// Every Link-destination packet leaves on the link's attached interface. The initiator's
/// out-link is bound when the link proof arrives, so a split Resource sent from the connector
/// must complete over broadcast-off too, not only the listener -> connector response
/// direction.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_initiator_sends_two_segments() {
    let pair = loopback_link_pair("initiator").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let payload = random_payload(MAX_EFFICIENT_SIZE + 64 * 1024);
    let resource_hash = pair
        .connector
        .transport
        .send_resource(&pair.link_id, payload.clone(), None)
        .await
        .expect("resource dispatched from the initiator side");

    let complete = assert_transfer_completes(
        &mut sender_events,
        &mut receiver_events,
        pair.link_id,
        resource_hash,
        &payload,
        2,
    )
    .await;
    assert!(!complete.is_request, "a plain resource is not a request");
    assert!(!complete.is_response, "a plain resource is not a response");
    assert!(complete.request_id.is_none(), "a plain resource carries no request id");
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;
}

/// Same direction, request form: a split request Resource from the initiator reaches the
/// destination side as one `Complete` flagged `is_request` with the request id intact.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_initiator_request_two_segments() {
    let pair = loopback_link_pair("request").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();

    let payload = random_payload(MAX_EFFICIENT_SIZE + 64 * 1024);
    let request_id = b"loopback-request".to_vec();
    let resource_hash = pair
        .connector
        .transport
        .send_request_resource(&pair.link_id, request_id.clone(), payload.clone(), None)
        .await
        .expect("request resource dispatched from the initiator side");

    let complete = assert_transfer_completes(
        &mut sender_events,
        &mut receiver_events,
        pair.link_id,
        resource_hash,
        &payload,
        2,
    )
    .await;
    assert!(complete.is_request, "advertised as a request");
    assert!(!complete.is_response, "not advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(request_id.as_slice()));
    assert_no_further_complete(&mut receiver_events, pair.link_id).await;
}

/// The proof handler hands the next advertisement back through a buffer it reuses across
/// calls. Two split transfers in a row on the same link must each finish exactly once,
/// with their own hash and bytes, and the link must stay usable for the second one.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_back_to_back_on_one_link() {
    let pair = loopback_link_pair("sequence").await;
    let mut sender_events = pair.listener.transport.resource_events();
    let mut receiver_events = pair.connector.transport.resource_events();

    let mut hashes = Vec::new();
    for (index, request_tag) in [&b"loopback-first"[..], &b"loopback-second"[..]]
        .into_iter()
        .enumerate()
    {
        let payload = random_payload(MAX_EFFICIENT_SIZE + 64 * 1024);
        solicit_response(&pair.connector.transport, &pair.link_id, request_tag).await;
        let resource_hash = pair
            .listener
            .transport
            .send_response_resource(&pair.link_id, request_tag.to_vec(), payload.clone(), None)
            .await
            .unwrap_or_else(|err| panic!("response resource {index} dispatched: {err:?}"));

        let complete = assert_transfer_completes(
            &mut sender_events,
            &mut receiver_events,
            pair.link_id,
            resource_hash,
            &payload,
            2,
        )
        .await;
        assert!(complete.is_response, "transfer {index} advertised as a response");
        assert_eq!(
            complete.request_id.as_deref(),
            Some(request_tag),
            "transfer {index} keeps its own request id"
        );
        assert_no_further_complete(&mut receiver_events, pair.link_id).await;
        hashes.push(resource_hash);
    }
    assert_ne!(hashes[0], hashes[1], "two random payloads hash differently");
}
