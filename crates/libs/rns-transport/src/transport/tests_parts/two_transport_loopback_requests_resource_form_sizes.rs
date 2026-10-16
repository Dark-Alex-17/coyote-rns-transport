// Loopback coverage for what a receipt reports about a response Resource and for the
// response-size gate. The status order follows `RNS.RequestReceipt` in both request forms
// and both response forms (`RNS/Link.py:1372`, `:1420`, `:1445-1450`); a split response
// reports `d` of its first advertisement as the response size, `t` summed over its
// segments as the transfer size and a monotone progress (`RNS/Link.py:1057-1061`,
// `:1429`); `max_response_size` is measured against `d` at every segment advertisement
// (`RNS/Link.py:1046-1049`, `RNS/Resource.py:1285-1287`), so a compressible response
// over the limit is refused however small its transfer, and the limit is not spent by an
// advertisement the ResourceManager drops; a response advertisement nobody solicited falls
// through Python's pending-request loop (`RNS/Link.py:1046-1047`) and is dropped here
// without a reject. Helpers come from the sibling `tests_parts` files.

/// Under `d` of an `over_mdu_zeros` envelope (a little over the link MDU, a quarter MiB
/// over the default loopback interface), far over its bz2 `t` of a few hundred bytes.
const SIZES_TIGHT_LIMIT: usize = 8 * 1024;

/// A response Resource this long splits into exactly three segments: `total_segments =
/// total.div_ceil(MAX_EFFICIENT_SIZE)` over the raw envelope.
const SIZES_THREE_SEGMENT_LEN: usize = 2 * MAX_EFFICIENT_SIZE + 64 * 1024;

/// A response Resource this long splits into exactly two segments.
const SIZES_TWO_SEGMENT_LEN: usize = MAX_EFFICIENT_SIZE + 64 * 1024;

/// How long an advertisement that must be dropped is given to be acted on instead.
const SIZES_SILENCE: Duration = Duration::from_secs(1);

/// The `[request_id, body]` envelope a `Bytes` response travels as, whose length is the
/// advertised `d`. The id packs to a fixed width, so any id gives the length.
fn sizes_envelope_len(body: &[u8]) -> usize {
    ResponseFrame::new(RequestId::new([0u8; ADDRESS_HASH_SIZE]), rmpv::Value::Binary(body.to_vec()))
        .encode()
        .len()
}

/// `response()` bounded by the Resource budget rather than the receipt one: a split
/// response takes longer than any single event.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_await_resource_receipt(
    receipt: &RequestReceipt,
) -> Result<RequestResponseData, RequestError> {
    timeout(RESOURCE_TIMEOUT, receipt.response()).await.expect("the split response concludes")
}

/// Samples `receipt.progress()` on every `Progress` and `SegmentComplete` event of the
/// pair's link until the response completes. Progress events outrun the receiver, so lag
/// is skipped.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_sample_progress(
    events: &mut broadcast::Receiver<ResourceEvent>,
    receipt: &RequestReceipt,
    link_id: AddressHash,
) -> Vec<f32> {
    let mut samples = Vec::new();
    timeout(RESOURCE_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.link_id == link_id => match &event.kind {
                    ResourceEventKind::Progress(_) | ResourceEventKind::SegmentComplete(_) => {
                        samples.push(receipt.progress());
                    }
                    ResourceEventKind::Complete(complete) if complete.is_response => return,
                    ResourceEventKind::InboundFailed(failure) => {
                        panic!("the response Resource failed: {failure:?}")
                    }
                    _ => {}
                },
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("resource event channel closed"),
            }
        }
    })
    .await
    .expect("the response Resource completes");
    samples
}

/// Nothing the connector would do with a response under `id` happens for `SIZES_SILENCE`:
/// no `Complete`, no `InboundFailed` on the link.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_assert_no_response_activity(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    id: RequestId,
) {
    let deadline = Instant::now() + SIZES_SILENCE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        match timeout(remaining, events.recv()).await {
            Ok(Ok(event)) if event.link_id == link_id => match &event.kind {
                ResourceEventKind::Complete(complete) => assert!(
                    complete.request_id.as_deref() != Some(id.as_bytes().as_slice()),
                    "the unsolicited response was assembled"
                ),
                ResourceEventKind::InboundFailed(failure) => {
                    panic!("an inbound Resource failed on the link: {failure:?}")
                }
                _ => {}
            },
            Ok(Ok(_)) => {}
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return,
        }
    }
}

/// A packet-form `/echo` on the pair's link still answers.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_assert_link_still_answers(pair: &LoopbackRequestPair) {
    let after = request(pair, "/echo", rmpv::Value::from("still here")).await;
    let answer = await_receipt(&after).await.expect("the link still answers");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("still here")));
}

/// Request Resource, response packet: `Sent -> Delivered -> Ready` (`RNS/Link.py:1372`,
/// `:1445-1450`), never `Receiving`, and both sizes read the packed body.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_packet_response_skips_receiving() {
    let pair = loopback_request_pair("sizes-resource-request-packet-response").await;
    register_handler(&pair, "/small", RequestAllow::All, |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::from("small"))))
    })
    .await;

    let body = over_mdu_body(&pair).await;
    let receipt = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    let seen = observe_statuses(&receipt).await;
    assert!(
        is_subsequence_of(
            &seen,
            &[RequestStatus::Sent, RequestStatus::Delivered, RequestStatus::Ready]
        ),
        "resource-form request, packet response: {seen:?}"
    );
    assert!(!seen.contains(&RequestStatus::Receiving), "a packet response is not received: {seen:?}");
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.value(), Some(&rmpv::Value::from("small")));
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, &rmpv::Value::from("small")).expect("packs");
    assert_eq!(receipt.response_size(), Some(packed.len()));
    assert_eq!(receipt.response_transfer_size(), Some(packed.len()));
}

/// Request Resource, response Resource: `Sent -> Delivered -> Receiving -> Ready`. Over the
/// legacy MTU both transfers take hundreds of parts, so `Delivered` and `Receiving` land on
/// the `watch` as separate changes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_resource_response_is_delivered_then_received(
) {
    let pair = loopback_request_pair_with_mtu("sizes-resource-request-resource-response", LEGACY_MTU)
        .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = random_payload(MANY_PARTS_LEN);
    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let seen = observe_statuses(&receipt).await;
    assert!(
        is_subsequence_of(
            &seen,
            &[
                RequestStatus::Sent,
                RequestStatus::Delivered,
                RequestStatus::Receiving,
                RequestStatus::Ready,
            ]
        ),
        "resource-form request, Resource response: {seen:?}"
    );
    assert!(seen.contains(&RequestStatus::Delivered), "the request Resource completes: {seen:?}");
    assert!(seen.contains(&RequestStatus::Receiving), "the echo comes back as a Resource: {seen:?}");
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
}

/// Request packet, response packet: `Sent -> Ready`; nothing proves a request packet.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_packet_request_packet_response_order() {
    let pair = loopback_request_pair("sizes-packet-request-packet-response").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from(1)).await;
    let seen = observe_statuses(&receipt).await;
    assert!(
        is_subsequence_of(&seen, &[RequestStatus::Sent, RequestStatus::Ready]),
        "packet request, packet response: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&RequestStatus::Ready));
    assert!(!seen.contains(&RequestStatus::Delivered), "{seen:?}");
    assert!(!seen.contains(&RequestStatus::Receiving), "{seen:?}");
}

/// Request packet, response Resource: `Sent -> Receiving -> Ready`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_packet_request_resource_response_order(
) {
    let pair = loopback_request_pair_with_mtu("sizes-packet-request-resource-response", LEGACY_MTU)
        .await;
    let body = random_payload(MANY_PARTS_LEN);
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let seen = observe_statuses(&receipt).await;
    assert!(
        is_subsequence_of(
            &seen,
            &[RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready]
        ),
        "packet request, Resource response: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&RequestStatus::Ready));
    assert!(seen.contains(&RequestStatus::Receiving), "{seen:?}");
    assert!(!seen.contains(&RequestStatus::Delivered), "{seen:?}");
}

/// A three-segment response: progress sampled on every receiver event stays within
/// `0.0..=1.0` and never decreases, ending at `1.0`; the three advertisements carry one
/// `d`, the whole envelope's length, which the receipt reports as the response size, and
/// their `t` add up to the transfer size (`RNS/Link.py:1057-1061`, `RNS/Resource.py:297`).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_three_segment_response_sizes_and_progress(
) {
    let mut pair = loopback_request_pair("sizes-three-segments").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(SIZES_THREE_SEGMENT_LEN);
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let samples = sizes_sample_progress(&mut connector_events, &receipt, link_id).await;
    assert!(!samples.is_empty(), "a split response reports progress");
    assert!(samples.iter().all(|sample| (0.0..=1.0).contains(sample)), "{samples:?}");
    assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]), "non-decreasing: {samples:?}");
    let response = sizes_await_resource_receipt(&receipt).await.expect("answered");
    assert_eq!(receipt.progress(), 1.0);
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected.clone())));

    let summary = drain_connector_tap(&mut pair).await;
    let advertisements = response_advertisements_for(&summary, receipt.request_id());
    let indices: Vec<u32> = advertisements.iter().map(|a| a.segment_index).collect();
    assert_eq!(indices, vec![1, 2, 3], "three segments advertised in order");
    assert!(advertisements.iter().all(|a| a.total_segments == 3), "{advertisements:?}");
    let envelope_len = sizes_envelope_len(&expected) as u64;
    assert!(
        advertisements.iter().all(|a| a.data_size == envelope_len),
        "every segment advertises the whole envelope as d: {advertisements:?}"
    );
    assert_eq!(receipt.response_size(), Some(envelope_len as usize), "d of the first advertisement");
    let transfer: u64 = advertisements.iter().map(|a| a.transfer_size).sum();
    assert_eq!(receipt.response_transfer_size(), Some(transfer as usize), "t summed over segments");
}

/// `RNS/Link.py:1049` reads `d`: a zeros envelope whose compressed `t` is far under
/// `max_response_size` is still refused when its `d` is over it. The receipt fails
/// `Oversize` without `Receiving`, the responder sees the reject, and the link stays usable.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_oversize_is_measured_by_data_size_not_transfer_size(
) {
    let mut pair = loopback_request_pair("sizes-oversize-by-d").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = over_mdu_zeros(&pair).await;
    let receipt = request_with(
        &pair,
        "/echo",
        rmpv::Value::Binary(body.clone()),
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(!seen.contains(&RequestStatus::Receiving), "refused at the advertisement: {seen:?}");

    let rejected = await_resource_event(&mut listener_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::OutboundRejected)
    })
    .await;
    assert!(matches!(rejected.kind, ResourceEventKind::OutboundRejected));

    let summary = drain_connector_tap(&mut pair).await;
    let [advertisement] = response_advertisements_for(&summary, receipt.request_id())[..] else {
        panic!("exactly one response advertisement names the request");
    };
    let envelope_len = sizes_envelope_len(&body) as u64;
    assert_eq!(advertisement.data_size, envelope_len, "d is the whole envelope");
    assert!(advertisement.data_size > SIZES_TIGHT_LIMIT as u64, "d is over the limit");
    assert!(
        advertisement.transfer_size < SIZES_TIGHT_LIMIT as u64,
        "t compressed under the limit: {}",
        advertisement.transfer_size
    );

    sizes_assert_link_still_answers(&pair).await;
}

/// The same zeros envelope under a limit that covers its `d` is admitted and resolves.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_within_limit_by_data_size_is_admitted()
{
    let pair = loopback_request_pair("sizes-within-limit-by-d").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = over_mdu_zeros(&pair).await;
    let limit = sizes_envelope_len(&body);
    let receipt = request_with(
        &pair,
        "/echo",
        rmpv::Value::Binary(body.clone()),
        RequestOptions::default().with_max_response_size(limit),
    )
    .await;
    let response = await_receipt(&receipt).await.expect("d == limit is admitted");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
    assert_eq!(receipt.response_size(), Some(limit));
}

/// `RNS/Link.py:1046-1047`: a response advertisement whose `q` matches no pending request
/// falls out of the loop, neither accepted nor rejected. Here it never reaches the
/// ResourceManager: the connector assembles nothing, fails nothing and sends no receiver
/// cancel, and the link goes on answering.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_unsolicited_response_is_dropped_silently(
) {
    let pair = loopback_request_pair("sizes-unsolicited-response").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let made_up = RequestId::new([0x5A; ADDRESS_HASH_SIZE]);
    pair.handles
        .listener
        .transport
        .send_response_resource(&link_id, made_up.to_bytes().to_vec(), random_payload(256), None)
        .await
        .expect("response resource dispatched");

    sizes_assert_no_response_activity(&mut connector_events, link_id, made_up).await;
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no reject goes back for an unsolicited response"
    );
}

/// The limit is checked against every segment's `d` and not spent by the first
/// advertisement: a two-segment response within `max_response_size` is admitted whole.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_limit_covers_every_segment_of_a_split_response(
) {
    let mut pair = loopback_request_pair("sizes-two-segments-within-limit").await;
    let body = random_payload(SIZES_TWO_SEGMENT_LEN);
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request_with(
        &pair,
        "/big",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TWO_SEGMENT_LEN + 1024),
    )
    .await;
    let response = sizes_await_resource_receipt(&receipt).await.expect("both segments admitted");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected)));

    let summary = drain_connector_tap(&mut pair).await;
    let advertisements = response_advertisements_for(&summary, receipt.request_id());
    let indices: Vec<u32> = advertisements.iter().map(|a| a.segment_index).collect();
    assert_eq!(indices, vec![1, 2], "both segments were advertised and admitted");
}
