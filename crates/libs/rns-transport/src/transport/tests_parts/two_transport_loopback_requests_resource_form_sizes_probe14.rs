// Usage-pattern coverage of the two kinds of failure event that can meet on one inbound
// Resource packet, written from the public contract: everything the ResourceManager queued
// before a packet is routed by that packet's drain and reaches the receipt it concerns,
// while the failure a cancel the link owes produces is published and never routed. The
// first test makes the packet the responder's advertisement for a response another receipt
// is waiting on, which is admitted: the queued failure fails its receipt `Receive` no later
// than the admitted response completes. The second adds a cancel the pair's link owes to a
// displaced record and makes the queued failure come from a second link of the same
// connector, so one drain carries both: the owed cancel goes out once, the other link's
// receipt fails `Receive`, the admitted response assembles. Helpers come from the sibling
// `tests_parts` files.

/// Sends `advertisement` from the responder over `link_id` as a resource advertisement
/// packet; `inject_advertisement_from_listener` for a link other than the pair's.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe14_inject_advertisement_on(
    pair: &LoopbackRequestPair,
    link_id: AddressHash,
    advertisement: &ResourceAdvertisement,
) {
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("the listener tracks the inbound link");
    let packet = {
        let link = in_link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &advertisement.pack().expect("the advertisement packs"),
        )
        .expect("the advertisement fits a link packet")
    };
    let outcome =
        pair.handles.listener.transport.send_link_packet_on_bound_iface(&in_link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the advertisement reaches the wire");
}

/// A `/quiet` receipt on `link_id` under `SIZES_TIGHT_LIMIT`, moved to `Receiving` by an
/// unbacked response advertisement injected on that link.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe14_receiving_receipt_on(
    pair: &LoopbackRequestPair,
    link_id: AddressHash,
) -> RequestReceipt {
    let receipt = pair
        .handles
        .connector
        .transport
        .request(
            &link_id,
            "/quiet",
            rmpv::Value::Nil,
            RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
        )
        .await
        .expect("request is sent");
    let mut status_rx = receipt.subscribe();
    let advertisement =
        sizes_cancel_unbacked_response_advertisement(receipt.request_id(), SIZES_CANCEL_ADMITTED_D);
    sizes_probe14_inject_advertisement_on(pair, link_id, &advertisement).await;
    await_status(&mut status_rx, RequestStatus::Receiving, RECEIPT_TIMEOUT).await;
    receipt
}

/// A receipt `Receiving` an unbacked response has its inbound Resource dropped, and the next
/// inbound packet on the link is the responder's advertisement for another receipt's
/// response, which is admitted and assembled. The failure queued by the drop reaches the
/// dropped receipt no later than the admitted response completes: `Err(Receive)`, `Failed`,
/// while the other receipt reads its body whole and the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe14_a_queued_failure_leaves_on_the_advertisement_another_receipt_admits(
) {
    let pair = loopback_request_pair("sizes-probe14-queued-failure-admitted-advert").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::Binary(served.clone()))))
    })
    .await;

    let (dropped, _dropped_rx) = sizes_cancel_probe_receiving_receipt(&pair).await;
    drop_resource_state(&pair.handles.connector.transport, pair.handles.link_id).await;

    // The next inbound Resource packet on the link is an advertisement the link admits.
    let answered = request(&pair, "/big", rmpv::Value::Nil).await;
    let response = await_receipt(&answered).await.expect("the admitted response completes");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
    assert_eq!(answered.status(), RequestStatus::Ready);

    assert_eq!(
        dropped.status(),
        RequestStatus::Failed,
        "the queued failure reached its receipt by the time the admitted response completed"
    );
    assert_eq!(await_receipt(&dropped).await, Err(RequestError::Receive));
    sizes_assert_link_still_answers(&pair).await;
}

/// One drain, both failure kinds. On the pair's link a receipt-less record receiving an
/// unbacked response is displaced by `expect_response(id, None)`, so the link owes that
/// transfer a cancel. On a second link of the same connector a receipt `Receiving` has its
/// inbound Resource dropped, which queues a failure in the shared manager. The pair link's
/// next inbound packet is the responder's advertisement for a third request's response:
/// the owed cancel leaves on it once and only its transfer is reported cancelled, the
/// second link's receipt fails `Receive`, the advertised response is admitted and read
/// whole, and nothing is cancelled on the second link, whose receiver was already gone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe14_an_owed_cancel_and_a_queued_failure_from_another_link_leave_on_one_admitting_advertisement(
) {
    let mut pair = loopback_request_pair("sizes-probe14-owed-cancel-and-queued-failure").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::Binary(served.clone()))))
    })
    .await;
    let mut events = pair.handles.connector.transport.resource_events();
    let (_sibling_link, sibling_link_id) = stop_open_sibling_link(&mut pair).await;

    let dropped = sizes_probe14_receiving_receipt_on(&pair, sibling_link_id).await;
    let (displaced, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(displaced.as_bytes(), None);
    drop_resource_state(&pair.handles.connector.transport, sibling_link_id).await;

    // The pair link's next inbound Resource packet: an advertisement the link admits.
    let answered = request(&pair, "/big", rmpv::Value::Nil).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        displaced_hash,
    )
    .await;
    let response = await_receipt(&answered).await.expect("the admitted response completes");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));

    // Within the loopback budget whichever drain finds the queued failure: the admitting
    // packet's, or the retry worker's if the packet's left it behind.
    assert_eq!(
        await_receipt(&dropped).await,
        Err(RequestError::Receive),
        "the other link's queued failure reaches its receipt"
    );
    assert_eq!(dropped.status(), RequestStatus::Failed);
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "the admitted response is not cancelled; the owed cancel went out once"
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
}
