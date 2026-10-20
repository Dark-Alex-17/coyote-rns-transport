// Usage-pattern coverage for the request receipts a consumer holds beside the broadcast
// streams and across the other requests on the same link. A `Duration::MAX` timeout never
// expires and leaves the link watchdog serving the link; one receipt timing out leaves the
// other receipts on the link, and the link itself, intact, and the failed receipt reads the
// same outcome on every later call; a link that closes while a response Resource is being
// received fails the receipt `LinkClosed`, not `Receive` (`RNS/Link.py:1435`); the
// `LinkEvent::Data(Response)` and `resource_events()` broadcasts still carry a response that
// resolved a receipt in-path. Helpers come from the sibling `tests_parts` files.

/// Two link checks plus scheduling slack: long enough for the watchdog to have looked at an
/// unbounded deadline more than once.
fn two_ticks_bound() -> Duration {
    INTERVAL_LINKS_CHECK * 2 + Duration::from_millis(500)
}

/// Tears the link down from the connector's side and sends the close packet to the listener.
#[deny(clippy::await_holding_invalid_type)]
async fn teardown_from_connector(pair: &LoopbackRequestPair) {
    let packet = pair.handles.link.lock().await.teardown().expect("an active link closes");
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "link close reaches the wire");
}

/// `Duration::MAX` is "never": the receipt stays `Sent` across several link checks without
/// the watchdog panicking on the arithmetic, the link keeps answering other requests, and a
/// link close still ends the receipt.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_unbounded_timeout_never_expires_and_keeps_the_link()
{
    let pair = loopback_request_pair("receipt-probe2-unbounded").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(Duration::MAX),
    )
    .await;
    assert_eq!(receipt.timeout(), Duration::MAX);

    assert!(
        timeout(two_ticks_bound(), receipt.response()).await.is_err(),
        "an unbounded receipt does not conclude on its own"
    );
    assert_eq!(receipt.status(), RequestStatus::Sent);
    assert_eq!(receipt.concluded_at(), None);

    let after = request(&pair, "/echo", rmpv::Value::from("watchdog alive")).await;
    let answer = await_receipt(&after).await.expect("the link still answers");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("watchdog alive")));
    assert_eq!(receipt.status(), RequestStatus::Sent, "the echo did not touch the quiet receipt");

    teardown_from_connector(&pair).await;
    assert_eq!(await_receipt(&receipt).await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
}

/// A timed-out receipt is one entry of the link's table: the request answered beside it
/// resolves `Ready`, a request sent after it resolves `Ready`, and the failed receipt reads
/// `Timeout` again on every later `response()` and `Failed` on a subscription taken after.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_timeout_leaves_the_other_requests_and_link_intact()
{
    let pair = loopback_request_pair("receipt-probe2-timeout-neighbours").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let started = std::time::Instant::now();
    let quiet = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    let beside = request(&pair, "/echo", rmpv::Value::from("beside")).await;

    let answer = await_receipt(&beside).await.expect("the neighbour is answered");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("beside")));
    assert_eq!(quiet.status(), RequestStatus::Sent, "the neighbour's answer is not ours");

    assert_times_out(&quiet, started).await;
    assert_eq!(await_receipt(&quiet).await, Err(RequestError::Timeout), "re-read is stable");
    assert_eq!(*quiet.subscribe().borrow(), RequestStatus::Failed);

    let after = request(&pair, "/echo", rmpv::Value::from("after")).await;
    let answer = await_receipt(&after).await.expect("the link still answers after a timeout");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("after")));
    assert_eq!(pair.handles.link.lock().await.status(), LinkStatus::Active);
}

/// `RNS/Link.py:1435`: a receipt that fails while a response Resource is inbound takes the
/// Resource down with it. The link closes from the requester's side mid-transfer, with the
/// responder stalled so the transfer cannot finish first; the receipt reads `LinkClosed`,
/// not the Resource's own `Receive`, and does not hang.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_link_close_while_receiving_fails_link_closed() {
    let pair = loopback_request_pair_with_mtu("receipt-probe2-close-receiving", LEGACY_MTU).await;
    let body = random_payload(LONG_RESPONSE_LEN);
    register_handler(&pair, "/long", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/long", rmpv::Value::Nil).await;
    let mut status_rx = receipt.subscribe();
    await_status(&mut status_rx, RequestStatus::Receiving, LOOPBACK_TIMEOUT).await;
    let stall = stall_transport(&pair.handles.listener.transport).await;
    assert!(receipt.progress() < 1.0, "the transfer is still under way");

    teardown_from_connector(&pair).await;

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(receipt.concluded_at().is_some());
    assert_eq!(pair.handles.link.lock().await.status(), LinkStatus::Closed);
    stall.await.expect("stall task finished");
}

/// A response packet that resolved a receipt in-path still posts `LinkEvent::Data(Response)`
/// on the connector's link events, carrying the same envelope.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_packet_response_still_posts_the_link_event() {
    let mut pair = loopback_request_pair("receipt-probe2-broadcast-packet").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request(&pair, "/echo", rmpv::Value::from("both ways")).await;
    let answer = await_receipt(&receipt).await.expect("the request is answered");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("both ways")));

    let body = await_response(&mut pair, receipt.request_id()).await;
    assert_eq!(body, rmpv::Value::from("both ways"), "the broadcast carries the same body");
}

/// A response Resource that resolved a receipt in-path still posts
/// `ResourceEventKind::Complete` with `is_response` and the request id on
/// `Transport::resource_events()`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_response_still_posts_complete() {
    let pair = loopback_request_pair("receipt-probe2-broadcast-resource").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = over_mdu_body(&pair).await;
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/big", rmpv::Value::Nil).await;
    let answer = await_receipt(&receipt).await.expect("the request is answered");
    assert_eq!(answer.into_value(), Some(rmpv::Value::Binary(expected.clone())));

    let complete = await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(receipt.request_id().as_bytes().as_slice()));
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, receipt.request_id().to_bytes());
    assert_eq!(value, rmpv::Value::Binary(expected));
}
