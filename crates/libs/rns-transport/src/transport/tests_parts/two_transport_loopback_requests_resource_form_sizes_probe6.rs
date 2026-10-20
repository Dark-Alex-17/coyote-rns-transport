// Usage-pattern coverage of `Link::set_response_size_limit` composed with the public request
// builders, written from its rustdoc contract: it works in either order with
// `send_request_resource*` and `request_packet*`; on a record already made it changes the
// limit alone, and a record made afterwards keeps the limit. Each case is told apart on the
// wire: a kept limit refuses an over-limit response at its advertisement and the responder
// sees `OutboundRejected` (`RNS/Link.py:1046-1049`); a lost limit would assemble it; a lost
// record would drop it without a reject. The boundary stays inclusive through this path.
// The post-assembly measurement of the CHANGELOG contract is then pinned for the two id
// forms no receipt owns: a response whose advertised `d` lied under the limit is assembled
// and withheld from `resource_events()`, the record is cleared, and the link still answers.
// Helpers come from the sibling `tests_parts` files.

/// Bytes the receipt-less records of this file are bounded to.
const SIZES_PROBE6_LIMIT: usize = 2048;

/// A decoy response Resource this far over `SIZES_TIGHT_LIMIT` is re-advertised under a lie.
const SIZES_PROBE6_LIE_EXCESS: usize = 64;

/// Sends `len` random bytes from the listener as a response under `request_id` and asserts
/// the responder sees it refused at its advertisement while the connector assembles nothing.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe6_assert_response_is_refused(
    handles: &LoopbackLinkHandles,
    events: &mut broadcast::Receiver<ResourceEvent>,
    responder_events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
    len: usize,
) {
    let refused_hash = handles
        .listener
        .transport
        .send_response_resource(&handles.link_id, request_id.to_vec(), random_payload(len), None)
        .await
        .expect("over-limit response resource dispatched");
    let outcome = await_resource_event(responder_events, |event| {
        event.hash == refused_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. }
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(outcome.kind, ResourceEventKind::OutboundRejected { .. }),
        "a response over the kept limit is refused at its advertisement, got {:?}",
        outcome.kind
    );
    sizes_assert_no_response_activity_under(events, handles.link_id, &[request_id]).await;
}

/// Sends a small request Resource under `request_id` with no limit of its own and waits for
/// its delivery.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe6_send_unbounded_request_resource(
    handles: &LoopbackLinkHandles,
    events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
) {
    let request_hash = handles
        .connector
        .transport
        .send_request_resource(&handles.link_id, request_id.to_vec(), random_payload(64), None)
        .await
        .expect("request resource dispatched");
    await_resource_event(events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
}

/// A limit set before the request Resource goes out without one of its own is kept by the
/// record the send makes: one byte over it is refused with a reject, exactly the limit is
/// delivered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe6_limit_set_before_the_request_resource_is_kept(
) {
    let handles = loopback_link_handles("sizes-probe6-limit-before-resource").await;
    let mut events = handles.connector.transport.resource_events();
    let mut responder_events = handles.listener.transport.resource_events();

    let refused_id = b"sizes-probe6-limit-first-over".to_vec();
    handles.link.lock().await.set_response_size_limit(&refused_id, SIZES_PROBE6_LIMIT);
    sizes_probe6_send_unbounded_request_resource(&handles, &mut events, &refused_id).await;
    sizes_probe6_assert_response_is_refused(
        &handles,
        &mut events,
        &mut responder_events,
        &refused_id,
        SIZES_PROBE6_LIMIT + 1,
    )
    .await;

    let served_id = b"sizes-probe6-limit-first-at".to_vec();
    handles.link.lock().await.set_response_size_limit(&served_id, SIZES_PROBE6_LIMIT);
    sizes_probe6_send_unbounded_request_resource(&handles, &mut events, &served_id).await;
    let complete =
        sizes_probe5_respond_and_complete(&handles, &mut events, &served_id, SIZES_PROBE6_LIMIT)
            .await;
    assert_eq!(complete.data.len(), SIZES_PROBE6_LIMIT, "exactly the limit is delivered");
}

/// A limit set after the request Resource was delivered is applied to the record the send
/// made, without ending it: one byte over is refused with a reject (not dropped as
/// unsolicited), exactly the limit is delivered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe6_limit_set_after_the_request_resource_is_kept(
) {
    let handles = loopback_link_handles("sizes-probe6-limit-after-resource").await;
    let mut events = handles.connector.transport.resource_events();
    let mut responder_events = handles.listener.transport.resource_events();

    let refused_id = b"sizes-probe6-record-first-over".to_vec();
    sizes_probe6_send_unbounded_request_resource(&handles, &mut events, &refused_id).await;
    handles.link.lock().await.set_response_size_limit(&refused_id, SIZES_PROBE6_LIMIT);
    sizes_probe6_assert_response_is_refused(
        &handles,
        &mut events,
        &mut responder_events,
        &refused_id,
        SIZES_PROBE6_LIMIT + 1,
    )
    .await;

    let served_id = b"sizes-probe6-record-first-at".to_vec();
    sizes_probe6_send_unbounded_request_resource(&handles, &mut events, &served_id).await;
    handles.link.lock().await.set_response_size_limit(&served_id, SIZES_PROBE6_LIMIT);
    let complete =
        sizes_probe5_respond_and_complete(&handles, &mut events, &served_id, SIZES_PROBE6_LIMIT)
            .await;
    assert_eq!(complete.data.len(), SIZES_PROBE6_LIMIT, "exactly the limit is delivered");
}

/// A limit set under the id of a bare `Link::request_packet` before the packet is sent is
/// kept when the send seam re-arms the record's clock: the handler's response Resource over
/// it is refused at its advertisement and the responder sees `OutboundRejected`; under a
/// limit of exactly the envelope length the same response is assembled; the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe6_limit_set_before_a_bare_request_packet_is_kept(
) {
    let pair = loopback_request_pair("sizes-probe6-limit-before-packet").await;
    let link_id = pair.handles.link_id;
    let body = over_mdu_body(&pair).await;
    let envelope_len = sizes_envelope_len(&body);
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(served.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();
    let mut responder_events = pair.handles.listener.transport.resource_events();

    let (packet, refused_id) = sizes_probe2_bare_request_packet(&pair, "/big").await;
    pair.handles.link.lock().await.set_response_size_limit(refused_id.as_bytes(), envelope_len - 1);
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert!(outcome.is_sent(), "request reaches the wire: {outcome:?}");
    let refused = await_resource_event(&mut responder_events, |event| {
        event.link_id == link_id
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected { .. }
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(refused.kind, ResourceEventKind::OutboundRejected { .. }),
        "the response over the limit set before the send is refused, got {:?}",
        refused.kind
    );
    sizes_assert_no_response_activity(&mut events, link_id, refused_id).await;

    let (packet, served_id) = sizes_probe2_bare_request_packet(&pair, "/big").await;
    pair.handles.link.lock().await.set_response_size_limit(served_id.as_bytes(), envelope_len);
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert!(outcome.is_sent(), "request reaches the wire: {outcome:?}");
    let complete = await_response_resource(&mut events, link_id, served_id).await;
    assert_eq!(complete.data.len(), envelope_len, "exactly the limit is delivered");
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, served_id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(body));

    sizes_assert_link_still_answers(&pair).await;
}

/// Advertises a real response Resource of `SIZES_TIGHT_LIMIT + SIZES_PROBE6_LIE_EXCESS` bytes
/// under a decoy id nothing solicited, captures that advertisement, and re-advertises the
/// same Resource under `request_id` with a data size under the limit. Returns the hash
/// under which the responder serves it, so the test can wait for the parts to be served.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe6_readvertise_under_a_lie(
    pair: &mut LoopbackRequestPair,
    request_id: &[u8],
) -> Hash {
    let decoy_id = b"sizes-probe6-decoy".to_vec();
    let metadata =
        rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("big.bin"))]);
    let mut packed_metadata = Vec::new();
    rmpv::encode::write_value(&mut packed_metadata, &metadata).expect("metadata packs");
    let served_hash = pair
        .handles
        .listener
        .transport
        .send_response_resource(
            &pair.handles.link_id,
            decoy_id.clone(),
            random_payload(SIZES_TIGHT_LIMIT + SIZES_PROBE6_LIE_EXCESS),
            Some(packed_metadata),
        )
        .await
        .expect("decoy response resource dispatched");
    let mut advertisement = sizes_probe5_capture_response_advertisement(pair, &decoy_id).await;
    assert!(
        advertisement.data_size as usize > SIZES_TIGHT_LIMIT,
        "the real advertisement is over the limit"
    );
    assert_eq!(advertisement.hash, served_hash, "the decoy is served under the hash sent");
    advertisement.request_id = Some(serde_bytes::ByteBuf::from(request_id.to_vec()));
    advertisement.data_size = (SIZES_TIGHT_LIMIT / 2) as u64;
    inject_advertisement_from_listener(pair, &advertisement).await;
    served_hash
}

/// Waits until the responder has finished serving `hash`, whatever way it ended.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe6_await_served(
    responder_events: &mut broadcast::Receiver<ResourceEvent>,
    hash: Hash,
) -> ResourceEventKind {
    await_resource_event(responder_events, |event| {
        event.hash == hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
                    | ResourceEventKind::OutboundRejected { .. }
            )
    })
    .await
    .kind
}

/// A receipt-less 16-byte record from `request_packet_with_max_response_size`: a response
/// whose advertisement claimed a size under the limit but assembles over it is measured
/// again when it completes, its `Complete` is withheld from `resource_events()`, and the
/// record is cleared, so a second response under the id is dropped without a reject; the
/// link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe6_lying_advertisement_under_a_receipt_less_packet_record_is_withheld(
) {
    let mut pair = loopback_request_pair("sizes-probe6-lie-packet-record").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();
    let mut responder_events = pair.handles.listener.transport.resource_events();

    let id =
        sizes_probe2_send_limited_request_packet(&pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let served_hash = sizes_probe6_readvertise_under_a_lie(&mut pair, id.as_bytes()).await;
    let served = sizes_probe6_await_served(&mut responder_events, served_hash).await;
    assert!(
        matches!(served, ResourceEventKind::OutboundComplete),
        "the lying advertisement is admitted on its claim and the parts are served, got {served:?}"
    );
    sizes_assert_no_response_activity(&mut events, link_id, id).await;
    sizes_probe5_assert_second_response_is_dropped(&pair.handles, &mut events, id.as_bytes()).await;
    sizes_assert_link_still_answers(&pair).await;
}

/// A legacy-length record from `send_request_resource_with_max_response_size`: the same lie
/// is caught at the post-assembly measurement, the `Complete` is withheld, the record is
/// cleared, and the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe6_lying_advertisement_under_a_legacy_id_record_is_withheld(
) {
    let mut pair = loopback_request_pair("sizes-probe6-lie-legacy-record").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();
    let mut responder_events = pair.handles.listener.transport.resource_events();

    let legacy_id = b"sizes-probe6-legacy-lie".to_vec();
    let request_hash = pair
        .handles
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &link_id,
            legacy_id.clone(),
            random_payload(64),
            None,
            Some(SIZES_TIGHT_LIMIT),
        )
        .await
        .expect("bounded request resource dispatched");
    await_resource_event(&mut events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;

    let served_hash = sizes_probe6_readvertise_under_a_lie(&mut pair, &legacy_id).await;
    let served = sizes_probe6_await_served(&mut responder_events, served_hash).await;
    assert!(
        matches!(served, ResourceEventKind::OutboundComplete),
        "the lying advertisement is admitted on its claim and the parts are served, got {served:?}"
    );
    sizes_assert_no_response_activity_under(&mut events, link_id, &[&legacy_id]).await;
    sizes_probe5_assert_second_response_is_dropped(&pair.handles, &mut events, &legacy_id).await;
    sizes_assert_link_still_answers(&pair).await;
}
