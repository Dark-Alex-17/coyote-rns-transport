// Usage-pattern coverage of the order between a cancel the link owes and the inbound
// packet it leaves on, written from the public contract: the cancel owed to a displaced
// response Resource is drained on the link's next inbound packet, Resource packets
// included, before that packet's own advertisement is admitted. The first test makes that
// packet the responder's retry of the very advertisement the displaced record was
// receiving (`RNS/Resource.py:594-598` re-advertises until a part request arrives): drained
// first, the cancel takes the old receiver down and the retry is admitted on its own merits
// under the fresh record; admitted first, the retry would be dropped as a duplicate of a
// receiver still active and the fresh record left idle. The second makes it the responder's
// own advertisement for another id, which the fresh record must still admit and assemble.
// Helpers come from the sibling `tests_parts` files.

/// Waits, within the loopback budget, for the connector to assemble a response under
/// `id` on the pair's link. An `InboundFailed` on the way fails the test: nothing else on
/// the link is dropped while the fresh advertisement is received.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe10_await_response_complete(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    id: RequestId,
) {
    timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match events.recv().await {
                Ok(event) if event.link_id == link_id => match &event.kind {
                    ResourceEventKind::Complete(complete)
                        if complete.is_response
                            && complete.request_id.as_deref() == Some(id.as_bytes().as_slice()) =>
                    {
                        return
                    }
                    ResourceEventKind::InboundFailed(failure) => {
                        panic!("an inbound Resource failed while {id} was received: {failure:?}")
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
    .expect("the fresh advertisement is admitted and its response assembled");
}

/// `expect_response(id, None)` over a receipt-less record receiving an unbacked response,
/// then the responder's retry of that same advertisement as the link's next inbound
/// packet: the displaced receiver is cancelled on that packet, once, before the retry is
/// admitted, so the retry builds a new receiver under the fresh record and asks for parts
/// again. The fresh record kept `SIZES_TIGHT_LIMIT` and is receiving: an advertisement over
/// the limit is refused and cancels that partial assembly too. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe10_owed_cancel_runs_before_a_retry_of_the_displaced_advertisement_is_admitted(
) {
    let pair = loopback_request_pair("sizes-probe10-retried-advertisement").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id =
        sizes_probe2_send_limited_request_packet(&pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let advertisement = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &advertisement).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;

    // A tap of its own for part requests: the receiver cancel wait below reads the shared
    // tap past whatever arrived with the cancel.
    let mut part_request_tap = pair.handles.listener.transport.iface_rx();
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(id.as_bytes(), None);

    // The responder has not seen a part request answered and advertises the transfer again.
    inject_advertisement_from_listener(&pair, &advertisement).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        advertisement.hash,
    )
    .await;
    await_link_packets(&mut part_request_tap, link_id, PacketContext::ResourceRequest, 1).await;

    // Over the kept limit: refused at its advertisement, and the record was receiving the
    // re-admitted transfer, so that partial assembly is cancelled with it.
    let over = sizes_cancel_unbacked_response_advertisement(id, SIZES_OVER_D);
    inject_advertisement_from_listener(&pair, &over).await;
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel, 1).await;
    sizes_probe8_await_inbound_failed(&mut events, link_id, id, advertisement.hash).await;

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no further receiver cancel leaves the connector"
    );
}

/// `expect_response(id, None)` over a receipt-less record receiving its response, then a
/// bare request under another id whose response is a Resource: the responder's
/// advertisement for it is the link's next inbound packet. The displaced transfer is
/// cancelled on that packet, once, and the advertisement is admitted under its own record
/// and assembled. The link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe10_owed_cancel_leaves_on_another_ids_advertisement_which_is_still_admitted(
) {
    let pair = loopback_request_pair("sizes-probe10-other-id-advertisement").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::Binary(body.clone()))))
    })
    .await;
    let mut events = pair.handles.connector.transport.resource_events();

    let (displaced, displaced_hash) = sizes_probe8_receiving_record(&pair, &mut listener_tap).await;
    let mut connector_tap = pair.handles.connector.transport.iface_rx();
    pair.handles.link.lock().await.expect_response(displaced.as_bytes(), None);

    let other = sizes_probe2_send_limited_request_packet(&pair, "/big", None).await;
    sizes_probe9_assert_displaced_transfer_cancelled(
        &pair,
        &mut events,
        &mut connector_tap,
        &mut listener_tap,
        displaced_hash,
    )
    .await;
    sizes_probe10_await_response_complete(&mut events, link_id, other).await;

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "the other id's response is not cancelled"
    );
}
