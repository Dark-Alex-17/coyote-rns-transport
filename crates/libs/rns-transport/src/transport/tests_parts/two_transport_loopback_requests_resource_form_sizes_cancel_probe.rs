// Usage-pattern coverage around the owed-cancel seam of a receipt already receiving a
// response Resource. A packet-form response is measured against `max_response_size` like
// any other response (`RNS/Link.py:862-863`, `:1016-1017`): within the limit it resolves
// the receipt `Ready` with its own sizes and nothing is cancelled (`:868-873`,
// `:1437-1443`); over it, only the receipt the packet names fails `Oversize`, only that
// receipt's response Resource is cancelled (`:876`, `:1435`), and a receipt with no
// response Resource inbound owes no cancel at all. Helpers come from the sibling
// `tests_parts` files.

/// A packet response the connector will measure as over `SIZES_TIGHT_LIMIT`.
fn sizes_cancel_probe_oversize_envelope(id: RequestId) -> Vec<u8> {
    ResponseFrame::new(id, rmpv::Value::Binary(vec![0u8; SIZES_TIGHT_LIMIT + 64])).encode()
}

/// A `/quiet` receipt under `SIZES_TIGHT_LIMIT` moved to `Receiving` by an unbacked
/// response advertisement of `d = SIZES_TIGHT_LIMIT / 2`, with its status subscription.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_cancel_probe_receiving_receipt(
    pair: &LoopbackRequestPair,
) -> (RequestReceipt, tokio::sync::watch::Receiver<RequestStatus>) {
    let receipt = request_with(
        pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    let mut status_rx = receipt.subscribe();
    let advertisement =
        sizes_cancel_unbacked_response_advertisement(receipt.request_id(), SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(pair, &advertisement).await;
    await_status(&mut status_rx, RequestStatus::Receiving, RECEIPT_TIMEOUT).await;
    (receipt, status_rx)
}

/// A packet response within `max_response_size` for a receipt already `Receiving` resolves
/// it `Ready` from the packet (`RNS/Link.py:873`, `:1437-1443`): the answer is the packet's,
/// the sizes become the packed body's (`:868-869`), progress is full, and no receiver cancel
/// goes back to the responder.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_cancel_probe_in_limit_packet_resolves_a_receiving_receipt_without_a_cancel(
) {
    let pair = loopback_request_pair("sizes-cancel-probe-in-limit-packet").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let (receipt, _status_rx) = sizes_cancel_probe_receiving_receipt(&pair).await;
    assert_eq!(
        receipt.response_size(),
        Some(SIZES_TIGHT_LIMIT / 2),
        "d of the admitted advertisement while receiving"
    );

    let answer = rmpv::Value::from("answered by a packet after all");
    let envelope = ResponseFrame::new(receipt.request_id(), answer.clone()).encode();
    sizes_cancel_send_response_packet_from_listener(&pair, &envelope).await;

    let response = await_receipt(&receipt).await.expect("the in-limit packet answers the receipt");
    assert_eq!(response.value(), Some(&answer));
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(receipt.progress(), 1.0);
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, &answer).expect("packs");
    assert_eq!(
        receipt.response_size(),
        Some(packed.len()),
        "the packet's packed body replaces the advertised d"
    );

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a receipt answered within its limit cancels nothing"
    );
}

/// Two receipts receiving on one link; an over-limit packet names only the first. The first
/// fails `Oversize`, the second stays `Receiving` with no status move, and exactly one
/// receiver cancel goes back: the cancel is keyed to the receipt the packet failed
/// (`RNS/Link.py:861`, `:1435`), not to every response Resource on the link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_cancel_probe_oversize_packet_cancels_only_the_named_receipts_resource(
) {
    let pair = loopback_request_pair("sizes-cancel-probe-two-receiving").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let (failing, _failing_rx) = sizes_cancel_probe_receiving_receipt(&pair).await;
    let (bystander, bystander_rx) = sizes_cancel_probe_receiving_receipt(&pair).await;
    assert_ne!(failing.request_id(), bystander.request_id(), "two distinct requests");

    sizes_cancel_send_response_packet_from_listener(
        &pair,
        &sizes_cancel_probe_oversize_envelope(failing.request_id()),
    )
    .await;

    assert_eq!(await_receipt(&failing).await, Err(RequestError::Oversize));
    assert_eq!(failing.status(), RequestStatus::Failed);
    sizes_assert_link_still_answers(&pair).await;

    assert_eq!(bystander.status(), RequestStatus::Receiving, "the other receipt is untouched");
    assert!(
        !bystander_rx.has_changed().expect("the bystander's entry is alive"),
        "no status move was posted for the other receipt"
    );
    assert_eq!(
        bystander.response_size(),
        Some(SIZES_TIGHT_LIMIT / 2),
        "the other receipt keeps its advertised d"
    );
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        1,
        "exactly the failed receipt's response Resource is cancelled"
    );
}

/// An over-limit packet response for a receipt still `Sent` fails it `Oversize`
/// (`RNS/Link.py:874-876`) with nothing to cancel: no response Resource was ever admitted
/// for it, so no receiver cancel goes back and the link goes on answering.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_cancel_probe_oversize_packet_for_a_sent_receipt_owes_no_cancel(
) {
    let pair = loopback_request_pair("sizes-cancel-probe-oversize-while-sent").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    assert_eq!(receipt.status(), RequestStatus::Sent);
    assert_eq!(receipt.response_size(), None, "nothing advertised yet");

    sizes_cancel_send_response_packet_from_listener(
        &pair,
        &sizes_cancel_probe_oversize_envelope(receipt.request_id()),
    )
    .await;

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a receipt with no response Resource inbound owes no cancel"
    );
}
