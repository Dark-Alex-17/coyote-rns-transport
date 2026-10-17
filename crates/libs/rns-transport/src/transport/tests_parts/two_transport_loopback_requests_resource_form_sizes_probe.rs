// Usage-pattern coverage for the response Resource path of a receipt, written from the README
// and CHANGELOG contract: a response Resource without metadata whose envelope names another
// request than the advertisement's `q` fails the receipt `Decode` while one that names the
// receipt's own id resolves it (the id is checked only on the Resource path, `RNS/Link.py:908-912`
// keys `handle_response` on the envelope's id); every `response()` result on a receipt and on
// its clones shares one body; a response Resource advertised under the id of a receipt that
// already resolved is unsolicited and is dropped like any other (`RNS/Link.py:1046-1047`), the
// receipt keeping its answer; a packet-form request whose response Resource is over
// `max_response_size` fails `Oversize` and the responder sees the reject (`RNS/Link.py:1049`);
// and the sizes of a receipt are `None` and its progress `0.0` until a response arrives.
// Helpers come from the sibling `tests_parts` files.

/// A response the probe sends by hand as a Resource: `[request_id, body]` packed, which is
/// what `Transport::send_response_resource` with no metadata is expected to carry.
fn sizes_probe_envelope(request_id: RequestId, body: &str) -> Vec<u8> {
    ResponseFrame::new(request_id, rmpv::Value::from(body)).encode()
}

/// `RNS/Link.py:908-912`: a response Resource without metadata is read as an envelope and the
/// id inside it picks the request. Here the advertisement (`q`) picks the receipt and the
/// envelope has to agree with it: one that names another request fails the receipt `Decode`,
/// without a reject back to the responder, and the link stays usable.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe_envelope_naming_another_request_fails_decode(
) {
    let pair = loopback_request_pair("sizes-probe-mismatched-envelope").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    assert_eq!(receipt.status(), RequestStatus::Sent);
    assert_eq!(receipt.response_size(), None, "no response has arrived");
    assert_eq!(receipt.response_transfer_size(), None, "no response has arrived");
    assert_eq!(receipt.progress(), 0.0, "nothing received yet");

    let other = RequestId::new([0x33; ADDRESS_HASH_SIZE]);
    assert_ne!(other, receipt.request_id());
    pair.handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            receipt.request_id().to_bytes().to_vec(),
            sizes_probe_envelope(other, "stray"),
            None,
        )
        .await
        .expect("response resource dispatched");

    let seen = observe_statuses(&receipt).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Decode));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(seen.last(), Some(&RequestStatus::Failed), "{seen:?}");
    assert!(
        is_subsequence_of(
            &seen,
            &[RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Failed]
        ),
        "a packet request whose response Resource does not decode: {seen:?}"
    );
    assert!(receipt.concluded_at().is_some(), "a failed receipt has concluded");

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a decode failure is the requester's own; nothing is rejected on the wire"
    );
}

/// The positive twin: the same hand-built envelope naming the receipt's own id resolves the
/// receipt `Ready` with the body, so the test above discriminates on the id alone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe_envelope_naming_the_receipt_resolves_it(
) {
    let pair = loopback_request_pair("sizes-probe-matching-envelope").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let envelope = sizes_probe_envelope(receipt.request_id(), "by hand");
    let envelope_len = envelope.len();
    pair.handles
        .listener
        .transport
        .send_response_resource(&link_id, receipt.request_id().to_bytes().to_vec(), envelope, None)
        .await
        .expect("response resource dispatched");

    let response = await_receipt(&receipt).await.expect("the matching envelope answers");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(response.value(), Some(&rmpv::Value::from("by hand")));
    assert_eq!(receipt.response_size(), Some(envelope_len), "d is the whole envelope");
    assert_eq!(receipt.progress(), 1.0);
}

/// `RequestResponseData` is a cheap handle: the results of every `response()` call on a
/// receipt and on its clones point at one body.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe_response_handles_share_one_body()
{
    let pair = loopback_request_pair("sizes-probe-shared-body").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let payload = random_payload(4 * 1024);
    let receipt = request(&pair, "/echo", rmpv::Value::Binary(payload.clone())).await;
    let clone = receipt.clone();
    let first = await_receipt(&receipt).await.expect("answered");
    let second = await_receipt(&receipt).await.expect("answered again");
    let third = await_receipt(&clone).await.expect("answered on the clone");
    let copied = first.clone();

    let body = first.value().expect("an envelope body");
    assert_eq!(body, &rmpv::Value::Binary(payload));
    for other in [&second, &third, &copied] {
        let other_body = other.value().expect("an envelope body");
        assert!(std::ptr::eq(body, other_body), "every handle shares the one body");
    }
    assert_eq!(first, second);
    assert_eq!(first, third);
}

/// A response Resource advertised under the `q` of a receipt that already resolved is as
/// unsolicited as one under a made-up id (`RNS/Link.py:1046-1047`): the connector assembles
/// nothing, sends no reject, the receipt keeps its answer and the link goes on answering.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe_late_response_under_a_resolved_id_is_dropped(
) {
    let pair = loopback_request_pair("sizes-probe-late-response").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from("first")).await;
    let answer = await_receipt(&receipt).await.expect("answered");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("first")));
    assert_eq!(receipt.status(), RequestStatus::Ready);

    pair.handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            receipt.request_id().to_bytes().to_vec(),
            sizes_probe_envelope(receipt.request_id(), "late"),
            None,
        )
        .await
        .expect("response resource dispatched");

    sizes_assert_no_response_activity(&mut connector_events, link_id, receipt.request_id()).await;
    assert_eq!(receipt.status(), RequestStatus::Ready, "the resolved receipt is untouched");
    assert_eq!(
        await_receipt(&receipt).await.expect("still answered").value(),
        Some(&rmpv::Value::from("first")),
        "the late response does not replace the answer"
    );
    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "no reject goes back for a response nobody is waiting for"
    );
}

/// A packet-form request whose response comes back as a Resource over `max_response_size`:
/// the receipt fails `Oversize` without `Receiving`, the responder sees `OutboundRejected`,
/// and the link stays usable (`RNS/Link.py:1049`). The legacy MTU makes the 64 KiB echo a
/// Resource response to a packet request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe_packet_request_oversize_resource_response_is_rejected(
) {
    let pair =
        loopback_request_pair_with_mtu("sizes-probe-packet-request-oversize", LEGACY_MTU).await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(MANY_PARTS_LEN);
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/big",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(!seen.contains(&RequestStatus::Receiving), "refused at the advertisement: {seen:?}");
    assert!(!seen.contains(&RequestStatus::Delivered), "a packet request is not proven: {seen:?}");
    assert_eq!(receipt.progress(), 0.0, "nothing of the refused response was received");

    let rejected = await_resource_event(&mut listener_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::OutboundRejected)
    })
    .await;
    assert!(matches!(rejected.kind, ResourceEventKind::OutboundRejected));

    sizes_assert_link_still_answers(&pair).await;
}
