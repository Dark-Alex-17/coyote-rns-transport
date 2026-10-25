// The four status orders of a `RequestReceipt`, pinned exactly end to end:
// `Sent -> Ready` for a packet request with a packet response, `Sent -> Receiving -> Ready`
// with a Resource response, and `Sent -> Delivered -> ...` for the same two when the request
// goes out as a Resource (`RNS/Link.py:1348`, `:1372`, `:1420`, `:1442`).
// `two_transport_loopback_requests_resource_form_sizes.rs` and the `receipt-order-*` tests
// of `two_transport_loopback_requests_receipt.rs` keep the subsequence form of these, which
// is all an undelayed responder guarantees: on the current-thread test runtime the request
// Resource's proof and the response can both be handled before the subscriber task runs,
// and a `watch` keeps only the latest value. Here the responder is gated on the subscriber
// having read the last status the request leg produces, so every change the request leg
// makes is read before the response leg makes the next one. Helpers come from the sibling
// `tests_parts` files.

/// Packet request, packet response: `Sent -> Ready`. Nothing proves a request packet, so a
/// packet-form receipt never reports `Delivered`; the response marks the underlying packet
/// receipt instead (`RNS/Link.py:1445-1450`). `Sent` is the status current at subscription,
/// so the gate opens as soon as the subscriber has read it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_packet_request_packet_response_is_sent_ready(
) {
    let pair = loopback_request_pair("orders-packet-packet").await;
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        gated_handler(gate.clone(), |request: InboundRequest| {
            RequestResponse::Value(request.into_data())
        }),
    )
    .await;

    let receipt = request(&pair, "/echo", rmpv::Value::from(1)).await;
    let seen = observe_statuses_releasing_at(&receipt, RequestStatus::Sent, &gate).await;
    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Ready]);
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.value(), Some(&rmpv::Value::from(1)));
}

/// Packet request, Resource response: `Sent -> Receiving -> Ready`, never `Delivered`
/// (`RNS/Link.py:1421-1427` mark only the packet receipt). `Receiving` lands on the response
/// advertisement (`:1420`) and `Ready` only once the many-part transfer completes, so the
/// two are far apart on their own; the gate keeps the responder the same as in the other
/// tests.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_packet_request_resource_response_is_sent_receiving_ready(
) {
    let pair = loopback_request_pair_with_mtu("orders-packet-resource", LEGACY_MTU).await;
    let body = random_payload(MANY_PARTS_LEN);
    let expected = body.clone();
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/big",
        RequestAllow::All,
        gated_handler(gate.clone(), move |_: InboundRequest| RequestResponse::Bytes(body.clone())),
    )
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let seen = observe_statuses_releasing_at(&receipt, RequestStatus::Sent, &gate).await;
    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready]);
    let response = await_receipt(&receipt).await.expect("answered");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected)));
}

/// Resource request, packet response: `Sent -> Delivered -> Ready`. `Delivered` lands when
/// the request Resource's proof completes it (`RNS/Link.py:1372`), `Ready` on the response
/// packet the gate lets through once `Delivered` has been read.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_requests_receipt_orders_resource_request_packet_response_is_sent_delivered_ready(
) {
    let pair = loopback_request_pair("orders-resource-packet").await;
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/small",
        RequestAllow::All,
        gated_handler(gate.clone(), |_: InboundRequest| {
            RequestResponse::Value(rmpv::Value::from("small"))
        }),
    )
    .await;

    let body = over_mdu_body(&pair).await;
    let receipt = request(&pair, "/small", rmpv::Value::Binary(body)).await;
    let seen = observe_statuses_releasing_at(&receipt, RequestStatus::Delivered, &gate).await;
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
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        gated_handler(gate.clone(), |request: InboundRequest| {
            RequestResponse::Value(request.into_data())
        }),
    )
    .await;

    let body = random_payload(MANY_PARTS_LEN);
    let receipt = request(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let seen = observe_statuses_releasing_at(&receipt, RequestStatus::Delivered, &gate).await;
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
