// Usage-pattern coverage of the per-link drain on the inbound packets a resource-form
// request draws, written from the public contract: everything the ResourceManager queued
// for a link before a packet on that link is routed by that packet's drain, while events
// queued for another link wait for the periodic drain, which routes them; the responder's
// part requests and its proof of the assembled request Resource are such packets. The
// responder is stalled while the request is sent, so the failure is queued only once the
// request is on the wire and before the responder has read it: the sending path's own
// drain has already run, and the first drains to see the failure are the ones those
// inbound packets carry. The answer is a small packet, which carries no manager drain of
// its own. Helpers come from the sibling `tests_parts` files.

/// How long the responder is kept from reading the request Resource's advertisement.
const SIZES_PROBE15_RESPONDER_STALL: Duration = Duration::from_millis(300);

/// The resource-form request's statuses pass `Delivered`, so the responder's proof was
/// handled on the pair's link, and end `Ready` on the small packet answer. The answer is held
/// on `gate` until `Delivered` is read, so the two cannot coalesce on the receipt's `watch`;
/// the subscription comes after the responder's stall, so `Delivered` may already be current.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe15_assert_proved_and_answered(
    pair: &LoopbackRequestPair,
    answered: &RequestReceipt,
    gate: &tokio::sync::Notify,
) {
    let seen = observe_statuses_releasing_at(answered, RequestStatus::Delivered, gate).await;
    assert!(
        seen.contains(&RequestStatus::Delivered),
        "the request Resource's proof reached the pair's link: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&RequestStatus::Ready), "the small answer arrived: {seen:?}");
    let mdu = pair.handles.link.lock().await.link_mdu();
    assert!(
        answered.response_size().is_some_and(|size| size <= mdu),
        "the answer came back as a packet: {:?}",
        answered.response_size()
    );
    assert_eq!(
        answered.response().await.map(|data| data.into_value()),
        Ok(Some(rmpv::Value::from("small")))
    );
}

/// A receipt `Receiving` on a sibling link of the same connector has its inbound Resource
/// dropped while a resource-form request on the pair's link is on the wire and the
/// responder has not read it yet. The part requests and the proof that follow on the pair's
/// link leave the sibling's queued failure where the periodic drain routes it: the sibling
/// receipt fails `Receive` within the loopback budget, the request is proved and answered,
/// no receiver cancel goes out on either link, and both links still send.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe15_part_requests_and_proof_on_one_link_leave_a_sibling_links_queued_failure_for_the_periodic_drain(
) {
    let mut pair = loopback_request_pair("sizes-probe15-proof-leaves-sibling-failure").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/small",
        RequestAllow::All,
        gated_handler(gate.clone(), |_| RequestResponse::Value(rmpv::Value::from("small"))),
    )
    .await;
    let body = over_mdu_body(&pair).await;
    let (_sibling_link, sibling_link_id) = stop_open_sibling_link(&mut pair).await;

    let dropped = sizes_probe14_receiving_receipt_on(&pair, sibling_link_id).await;

    // The responder cannot read the advertisement until the stall ends, so the failure
    // queued below precedes every inbound packet the request draws on the pair's link.
    let stall =
        stall_transport_for(&pair.handles.listener.transport, SIZES_PROBE15_RESPONDER_STALL).await;
    let answered = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    drop_resource_state(&pair.handles.connector.transport, sibling_link_id).await;
    assert_eq!(dropped.status(), RequestStatus::Receiving, "the drop queues, it does not route");
    stall.await.expect("stall task finished");

    sizes_probe15_assert_proved_and_answered(&pair, &answered, &gate).await;

    assert_eq!(
        await_receipt(&dropped).await,
        Err(RequestError::Receive),
        "the sibling link's queued failure reaches its receipt"
    );
    assert_eq!(dropped.status(), RequestStatus::Failed);
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "nothing on the pair's link was cancelled"
    );
    assert_eq!(
        count_link_packets(
            &mut listener_tap,
            sibling_link_id,
            PacketContext::ResourceReceiverCancel
        ),
        0,
        "a receiver already dropped has nothing left to cancel"
    );
    sizes_assert_link_still_answers(&pair).await;
    let sibling_request = pair
        .handles
        .connector
        .transport
        .request(&sibling_link_id, "/quiet", rmpv::Value::Nil, RequestOptions::default())
        .await
        .expect("the sibling link still sends");
    assert_eq!(sibling_request.status(), RequestStatus::Sent);
}
