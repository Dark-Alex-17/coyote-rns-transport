// The four status orders of a `RequestReceipt`, pinned exactly end to end:
// `Sent -> Ready` for a packet request with a packet response, `Sent -> Receiving -> Ready`
// with a Resource response, and `Sent -> Delivered -> ...` for the same two when the request
// goes out as a Resource (`RNS/Link.py:1348`, `:1372`, `:1420`, `:1442`).
// `two_transport_loopback_requests_resource_form_sizes.rs` keeps the subsequence form of
// these, which is all an undelayed responder guarantees: on the current-thread test runtime
// the request Resource's proof and the response can both be handled before the subscriber
// task runs, and a `watch` keeps only the latest value. Here the responder waits
// `RECEIPT_ORDERS_GAP` before answering, so every change the request leg makes is read
// before the response leg makes the next one. Helpers come from the sibling `tests_parts`
// files.

/// How long the responder waits before answering, so each status change reaches the
/// subscriber before the next one replaces it on the `watch`.
const RECEIPT_ORDERS_GAP: Duration = Duration::from_millis(500);

/// A handler that answers `response(request)` after `RECEIPT_ORDERS_GAP`.
fn receipt_orders_delayed(
    response: impl Fn(InboundRequest) -> RequestResponse + Send + Sync + Clone + 'static,
) -> impl Fn(
    InboundRequest,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<RequestResponse>> + Send>>
       + Send
       + Sync {
    move |request: InboundRequest| {
        let response = response.clone();
        Box::pin(async move {
            tokio::time::sleep(RECEIPT_ORDERS_GAP).await;
            Some(response(request))
        })
    }
}

/// Packet request, packet response: `Sent -> Ready`. Nothing proves a request packet, so a
/// packet-form receipt never reports `Delivered`; the response marks the underlying packet
/// receipt instead (`RNS/Link.py:1445-1450`).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_packet_request_packet_response_is_sent_ready(
) {
    let pair = loopback_request_pair("orders-packet-packet").await;
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        receipt_orders_delayed(|request: InboundRequest| {
            RequestResponse::Value(request.into_data())
        }),
    )
    .await;

    let receipt = request(&pair, "/echo", rmpv::Value::from(1)).await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Ready]);
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.value(), Some(&rmpv::Value::from(1)));
}

/// Packet request, Resource response: `Sent -> Receiving -> Ready`, never `Delivered`
/// (`RNS/Link.py:1421-1427` mark only the packet receipt). `Receiving` lands on the response
/// advertisement (`:1420`) and `Ready` only once the many-part transfer completes, so the
/// two are far apart on their own; the delay keeps the responder the same as in the other
/// tests.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_packet_request_resource_response_is_sent_receiving_ready(
) {
    let pair = loopback_request_pair_with_mtu("orders-packet-resource", LEGACY_MTU).await;
    let body = random_payload(MANY_PARTS_LEN);
    let expected = body.clone();
    register_handler(
        &pair,
        "/big",
        RequestAllow::All,
        receipt_orders_delayed(move |_: InboundRequest| RequestResponse::Bytes(body.clone())),
    )
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready]);
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected)));
}

/// Resource request, packet response: `Sent -> Delivered -> Ready`. `Delivered` lands when
/// the request Resource's proof completes it (`RNS/Link.py:1372`), `Ready` on the response
/// packet `RECEIPT_ORDERS_GAP` later.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_resource_request_packet_response_is_sent_delivered_ready(
) {
    let pair = loopback_request_pair("orders-resource-packet").await;
    register_handler(
        &pair,
        "/small",
        RequestAllow::All,
        receipt_orders_delayed(|_: InboundRequest| {
            RequestResponse::Value(rmpv::Value::from("small"))
        }),
    )
    .await;

    let body = over_mdu_body(&pair).await;
    let receipt = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Delivered, RequestStatus::Ready]);
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.value(), Some(&rmpv::Value::from("small")));
}

/// Resource request, Resource response: `Sent -> Delivered -> Receiving -> Ready`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_resource_request_resource_response_is_sent_delivered_receiving_ready(
) {
    let pair = loopback_request_pair_with_mtu("orders-resource-resource", LEGACY_MTU).await;
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        receipt_orders_delayed(|request: InboundRequest| {
            RequestResponse::Value(request.into_data())
        }),
    )
    .await;

    let body = random_payload(MANY_PARTS_LEN);
    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let seen = observe_statuses(&receipt).await;
    assert_eq!(
        seen,
        vec![
            RequestStatus::Sent,
            RequestStatus::Delivered,
            RequestStatus::Receiving,
            RequestStatus::Ready,
        ]
    );
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
}
