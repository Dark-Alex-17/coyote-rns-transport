// Loopback coverage for the requester side of the request layer: `Transport::request` and the
// `RequestReceipt` it returns (`RNS/Link.py:473-510`, `:1311-1500`). A response that names a
// pending request resolves the receipt in-path, as a packet or as a Resource, on every clone of
// the handle; the status order follows Python's `RequestReceipt` (packet form never reaches
// `Delivered`, the resource form does on its Resource's completion, `RNS/Link.py:1372`); a link
// that closes fails every pending receipt. Helpers come from the sibling `tests_parts` files.

use crate::destination::request::{
    RequestError, RequestOptions, RequestReceipt, RequestResponseData, RequestStatus,
};

/// Bounds a `response()` wait that should resolve on its own.
const RECEIPT_TIMEOUT: Duration = LOOPBACK_TIMEOUT;

/// The original Reticulum interface MTU; a link over it carries a few hundred bytes per
/// packet, so a Resource of some tens of KiB takes many parts and a measurable time.
const LEGACY_MTU: usize = crate::destination::link::LEGACY_RETICULUM_MTU;

/// A response body that takes a few hundred parts over `LEGACY_MTU`.
const MANY_PARTS_LEN: usize = 64 * 1024;

/// Long enough to tell two successive status changes apart on the `watch`, short enough not
/// to approach any timeout.
const HANDLER_DELAY: Duration = Duration::from_millis(300);

/// A handler that answers nothing, so every request to it stays pending (`RNS/Link.py:844`).
fn silent_handler(
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    |_: InboundRequest| std::future::ready(None::<RequestResponse>)
}

/// Sends `value` to `path` through `Transport::request` with `opts`.
#[deny(clippy::await_holding_invalid_type)]
async fn request_with(
    pair: &LoopbackRequestPair,
    path: &str,
    value: rmpv::Value,
    opts: RequestOptions,
) -> RequestReceipt {
    pair.handles
        .connector
        .transport
        .request(&pair.handles.link_id, path, value, opts)
        .await
        .expect("request is sent")
}

#[deny(clippy::await_holding_invalid_type)]
async fn request(pair: &LoopbackRequestPair, path: &str, value: rmpv::Value) -> RequestReceipt {
    request_with(pair, path, value, RequestOptions::default()).await
}

/// `response()` on the receipt, bounded.
#[deny(clippy::await_holding_invalid_type)]
async fn await_receipt(receipt: &RequestReceipt) -> Result<RequestResponseData, RequestError> {
    timeout(RECEIPT_TIMEOUT, receipt.response()).await.expect("receipt concludes")
}

/// `response()` on the receipt and on a clone taken before it concluded, both resolving to
/// the same `Ok` body, with the receipt reporting `Ready` and full progress.
#[deny(clippy::await_holding_invalid_type)]
async fn await_ready_on_two_clones(receipt: &RequestReceipt) -> RequestResponseData {
    let clone = receipt.clone();
    let body = await_receipt(receipt).await.expect("the request is answered");
    assert_eq!(await_receipt(&clone).await, Ok(body.clone()), "the clone reads the same body");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(clone.status(), RequestStatus::Ready);
    assert_eq!(receipt.progress(), 1.0);
    assert!(receipt.concluded_at().is_some(), "a ready receipt has concluded");
    body
}

/// Every status a subscriber sees, from the one current when it subscribes until the receipt
/// concludes. A `watch` coalesces quick successive values, so the result is a subsequence of
/// the full order.
#[deny(clippy::await_holding_invalid_type)]
async fn observe_statuses(receipt: &RequestReceipt) -> Vec<RequestStatus> {
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];
    timeout(RECEIPT_TIMEOUT, async {
        while !matches!(seen.last(), Some(RequestStatus::Ready | RequestStatus::Failed)) {
            status_rx.changed().await.expect("the pending entry outlives the subscription");
            seen.push(*status_rx.borrow_and_update());
        }
    })
    .await
    .expect("the receipt concludes");
    seen
}

/// `seen` is `expected` with zero or more entries skipped, in order.
fn is_subsequence_of(seen: &[RequestStatus], expected: &[RequestStatus]) -> bool {
    let mut expected = expected.iter();
    seen.iter().all(|status| expected.any(|candidate| candidate == status))
}

/// Tears the link down from the responder's side and sends the close packet to the connector.
#[deny(clippy::await_holding_invalid_type)]
async fn teardown_from_listener(pair: &LoopbackRequestPair) {
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("the listener tracks the inbound link");
    let packet = in_link.lock().await.teardown().expect("an active link builds a close packet");
    let outcome =
        pair.handles.listener.transport.send_link_packet_on_bound_iface(&in_link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "link close reaches the wire");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_packet_response_resolves_every_clone() {
    let mut pair = loopback_request_pair("receipt-packet-packet").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from("ping")).await;

    let body = await_ready_on_two_clones(&receipt).await;
    assert_eq!(body.value(), Some(&rmpv::Value::from("ping")));
    assert_eq!(body.data(), None, "an envelope response is not a file response");
    assert_eq!(body.metadata(), None);
    // `RNS/Link.py:1016-1017`: a packet response reports its packed body as both sizes.
    let mut packed_body = Vec::new();
    rmpv::encode::write_value(&mut packed_body, &rmpv::Value::from("ping")).expect("packs");
    assert_eq!(receipt.response_size(), Some(packed_body.len()));
    assert_eq!(receipt.response_transfer_size(), Some(packed_body.len()));

    // The broadcast keeps carrying the response for consumers that correlate it themselves.
    let link_id = pair.handles.link_id;
    let id = receipt.request_id();
    await_event(&mut pair.out_events, |event| {
        event.id == link_id
            && matches!(&event.event, LinkEvent::Data(payload)
                if payload.context() == PacketContext::Response
                    && payload.request_id() == Some(id.to_bytes()))
    })
    .await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_response_resolves_with_sizes() {
    let pair = loopback_request_pair("receipt-packet-resource").await;
    let body = over_mdu_body(&pair).await;
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let response = await_ready_on_two_clones(&receipt).await;
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected.clone())));
    let response_size = receipt.response_size().expect("the advertisement carried `d`");
    assert!(response_size > expected.len(), "`d` covers the whole envelope: {response_size}");
    assert!(receipt.response_transfer_size().is_some(), "the advertisement carried `t`");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_form_request_resolves() {
    let pair = loopback_request_pair("receipt-resource-form").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    register_handler(&pair, "/small", RequestAllow::All, |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::from("small"))))
    })
    .await;
    let body = over_mdu_body(&pair).await;

    let echoed = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let response = await_ready_on_two_clones(&echoed).await;
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body.clone())));
    assert!(echoed.response_size().is_some(), "the echo came back as a Resource");

    let answered_small = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    let response = await_ready_on_two_clones(&answered_small).await;
    assert_eq!(response.value(), Some(&rmpv::Value::from("small")));
    let mdu = pair.handles.link.lock().await.link_mdu();
    assert!(
        answered_small.response_size().is_some_and(|size| size <= mdu),
        "the answer came back as a packet: {:?}",
        answered_small.response_size()
    );
}

/// `RNS/Link.py:899-903`: a response Resource with metadata is a file response, its data the
/// raw bytes and its metadata unpacked beside them.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_file_response_carries_data_and_metadata() {
    let pair = loopback_request_pair("receipt-file-response").await;
    let data = random_payload(64);
    let expected_data = data.clone();
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("a.bin"))]);
    let expected_metadata = metadata.clone();
    register_handler(&pair, "/file", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Resource {
            data: data.clone(),
            metadata: Some(metadata.clone()),
        }))
    })
    .await;

    let receipt = request(&pair, "/file", rmpv::Value::Nil).await;
    let response = await_ready_on_two_clones(&receipt).await;
    assert_eq!(response.data(), Some(expected_data.as_slice()));
    assert_eq!(response.metadata(), Some(&expected_metadata));
    assert_eq!(response.value(), None, "a file response has no envelope body");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_packet_form_goes_sent_to_ready() {
    let pair = loopback_request_pair("receipt-order-packet").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from(1)).await;
    let seen = observe_statuses(&receipt).await;
    assert!(
        is_subsequence_of(&seen, &[RequestStatus::Sent, RequestStatus::Ready]),
        "packet request, packet response: {seen:?}"
    );
    assert_eq!(seen.last(), Some(&RequestStatus::Ready));
    assert!(!seen.contains(&RequestStatus::Delivered), "nothing proves a request packet");
    assert!(!seen.contains(&RequestStatus::Receiving), "a packet response is not received in parts");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_response_goes_through_receiving() {
    let pair = loopback_request_pair_with_mtu("receipt-order-resource-response", LEGACY_MTU).await;
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
    assert!(seen.contains(&RequestStatus::Receiving), "a Resource response is received: {seen:?}");
    assert!(!seen.contains(&RequestStatus::Delivered), "nothing proves a request packet");
}

/// The echo waits before answering, so `Delivered` (the request Resource's proof) and
/// `Receiving` (the response advertisement) cannot land on the `watch` as one change.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_form_goes_through_delivered() {
    let pair = loopback_request_pair("receipt-order-resource-form").await;
    register_handler(&pair, "/echo", RequestAllow::All, |request: InboundRequest| async move {
        tokio::time::sleep(HANDLER_DELAY).await;
        Some(RequestResponse::Value(request.into_data()))
    })
    .await;
    let body = over_mdu_body(&pair).await;

    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body)).await;
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
    assert_eq!(seen.last(), Some(&RequestStatus::Ready));
    assert!(seen.contains(&RequestStatus::Delivered), "the request Resource completes: {seen:?}");
    assert!(seen.contains(&RequestStatus::Receiving), "the echo comes back as a Resource: {seen:?}");
}

/// `RNS/Link.py:874-876`: a response over `max_response_size` is rejected and the receipt
/// fails.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_oversize_packet_response_fails_oversize() {
    let pair = loopback_request_pair("receipt-oversize").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/echo",
        rmpv::Value::from("far more than one byte"),
        RequestOptions::default().with_max_response_size(1),
    )
    .await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Oversize));
    assert_eq!(receipt.status(), RequestStatus::Failed);
}

/// `RNS/Link.py:1036-1046`, `:1377-1385`: a request Resource over the destination's
/// `max_request_size` is refused at its advertisement and the receipt fails `Rejected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_refused_request_resource_fails_rejected() {
    let pair = loopback_request_pair("receipt-rejected").await;
    pair.handles
        .destination
        .lock()
        .await
        .set_max_request_size(RESOURCE_FORM_REQUEST_LIMIT)
        .expect("limit is set");
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    // Over the link MDU, so the request leaves as a Resource, and over the limit.
    let mdu = pair.handles.link.lock().await.link_mdu();
    let body = random_payload(mdu.max(RESOURCE_FORM_REQUEST_LIMIT) + 1);
    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body)).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Rejected));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(receipt.started_at(), None, "a refused request Resource never started a clock");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_connector_close_fails_every_pending_receipt() {
    let pair = loopback_request_pair("receipt-close-connector").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let first = request(&pair, "/quiet", rmpv::Value::from(1)).await;
    let second = request(&pair, "/quiet", rmpv::Value::from(2)).await;
    let second_clone = second.clone();

    let packet = pair.handles.link.lock().await.teardown().expect("an active link closes");
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "link close reaches the wire");

    assert_eq!(await_receipt(&first).await, Err(RequestError::LinkClosed));
    assert_eq!(await_receipt(&second).await, Err(RequestError::LinkClosed));
    assert_eq!(await_receipt(&second_clone).await, Err(RequestError::LinkClosed));
    assert_eq!(first.status(), RequestStatus::Failed);
    assert_eq!(second.status(), RequestStatus::Failed);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_listener_close_fails_every_pending_receipt() {
    let pair = loopback_request_pair("receipt-close-listener").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let first = request(&pair, "/quiet", rmpv::Value::from(1)).await;
    let second = request(&pair, "/quiet", rmpv::Value::from(2)).await;

    teardown_from_listener(&pair).await;

    assert_eq!(await_receipt(&first).await, Err(RequestError::LinkClosed));
    assert_eq!(await_receipt(&second).await, Err(RequestError::LinkClosed));
    assert_eq!(pair.handles.link.lock().await.status(), LinkStatus::Closed);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_unknown_link_is_an_invalid_argument() {
    let pair = loopback_request_pair("receipt-unknown-link").await;
    let unknown = AddressHash::new_from_rand(OsRng);

    let result = pair
        .handles
        .connector
        .transport
        .request(&unknown, "/echo", rmpv::Value::Nil, RequestOptions::default())
        .await;
    assert!(matches!(result, Err(RnsError::InvalidArgument)), "{result:?}");
}
