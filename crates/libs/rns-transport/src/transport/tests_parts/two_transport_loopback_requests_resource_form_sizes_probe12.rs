// Usage-pattern coverage of the receipt-less twin of `..._sizes_cancel_probe.rs`: a packet
// response within `max_response_size` for a record already receiving its response Resource.
// Written from the public contract: the packet is measured like any other response
// (`RNS/Link.py:862-863`), within the limit it answers the request and is posted as
// `LinkEvent::Data(Response)`, and the inbound Resource is left to its own retry watchdog
// rather than cancelled (`RNS/Link.py:873`, `:1437-1443`), so no receiver cancel goes back
// to the responder. The record is spent by the answer, so a later advertisement under the
// id is one the link no longer solicits and is dropped without a reject
// (`RNS/Link.py:1046-1056`). When the Resource the record was receiving is a real one, it
// still assembles and its `Complete` reaches `resource_events()`: nothing withholds a
// response no record bounds. Helpers come from the sibling `tests_parts` files.

/// Bytes of the real response Resource the second test receives over the legacy MTU: many
/// parts, so a packet response can land while it is still inbound.
const SIZES_PROBE12_BIG_RESPONSE: usize = 256 * 1024;

/// Sends `answer` from the responder as a packet response under `id` and asserts the
/// connector posts it on its link events, naming `id`.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe12_answer_with_a_packet(
    pair: &mut LoopbackRequestPair,
    id: RequestId,
    answer: &rmpv::Value,
) {
    let envelope = ResponseFrame::new(id, answer.clone()).encode();
    sizes_cancel_send_response_packet_from_listener(pair, &envelope).await;
    let body = await_response(pair, id).await;
    assert_eq!(&body, answer, "the in-limit packet answers the receipt-less request");
}

/// Nothing under `id` has completed on `events` yet: the response Resource is still
/// inbound, which is the premise of answering it with a packet.
fn sizes_probe12_assert_still_inbound(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    id: RequestId,
) {
    while let Ok(event) = events.try_recv() {
        if event.link_id == link_id {
            if let ResourceEventKind::Complete(complete) = &event.kind {
                assert_ne!(
                    complete.request_id.as_deref(),
                    Some(id.as_bytes().as_slice()),
                    "premise: the response Resource is still inbound when the packet is sent"
                );
            }
        }
    }
}

/// A receipt-less record under `SIZES_TIGHT_LIMIT` receiving an unbacked response: a packet
/// response within the limit answers it, is posted on the link events, and cancels nothing;
/// the inbound Resource stays with its retry watchdog. The record is spent: an
/// advertisement over the limit under the same id is dropped without a reject, where a
/// record still receiving would have refused it with a receiver cancel. The link still
/// answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe12_in_limit_packet_answers_a_receiving_receipt_less_record_without_a_cancel(
) {
    let mut pair = loopback_request_pair("sizes-probe12-in-limit-packet-receipt-less").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let (id, _receiving_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;

    let answer = rmpv::Value::from("answered by a packet after all");
    sizes_probe12_answer_with_a_packet(&mut pair, id, &answer).await;

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a record answered within its limit cancels nothing: the inbound Resource is left to \
         its retry watchdog"
    );

    // The answer spent the record: an over-limit advertisement under the id is no longer
    // solicited, so it is dropped, not refused. A record still receiving would answer it with
    // a receiver cancel for the advertised transfer and another for the partial assembly.
    let over = sizes_cancel_unbacked_response_advertisement(id, SIZES_TIGHT_LIMIT as u64 + 1);
    inject_advertisement_from_listener(&pair, &over).await;
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "an advertisement under a spent id is dropped as unsolicited, without a reject"
    );
}

/// A receipt-less record whose response is a real many-part Resource over the legacy MTU:
/// a packet response within the limit, sent while the Resource is inbound, answers the
/// request on the link events, the Resource goes on to assemble and its `Complete` reaches
/// `resource_events()` naming the id, and no receiver cancel goes back. The link still
/// answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe12_in_limit_packet_leaves_a_real_inbound_response_to_complete(
) {
    let mut pair =
        loopback_request_pair_with_mtu("sizes-probe12-in-limit-packet-real-resource", LEGACY_MTU)
            .await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = random_payload(SIZES_PROBE12_BIG_RESPONSE);
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id = sizes_probe2_send_limited_request_packet(
        &pair,
        "/big",
        Some(4 * SIZES_PROBE12_BIG_RESPONSE),
    )
    .await;
    // The connector asks for parts only once the advertisement is admitted under the record.
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;
    sizes_probe12_assert_still_inbound(&mut events, link_id, id);

    let answer = rmpv::Value::from("answered by a packet while the resource is inbound");
    sizes_probe12_answer_with_a_packet(&mut pair, id, &answer).await;

    sizes_probe10_await_response_complete(&mut events, link_id, id).await;
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "the inbound response Resource is left alone and completes"
    );
}
