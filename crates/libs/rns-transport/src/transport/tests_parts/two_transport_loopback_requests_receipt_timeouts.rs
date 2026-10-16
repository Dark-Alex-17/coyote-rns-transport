// Loopback coverage for the request timeout and the other ways a pending receipt ends without
// a response. The default is `rtt * 6 + 11.25 s` (`RNS/Link.py:491`), `RequestOptions::
// with_timeout` overrides it, and the link watchdog fails an unanswered receipt with
// `RequestError::Timeout` once the clock runs out. The clock runs from the send for a request
// packet (`RNS/Link.py:1331-1334`) and from `Delivered` for a request Resource (`:1370-1373`);
// it does not run while a response Resource is being received (`:1389`), whose own failure
// fails the receipt `Receive` instead. Python (`:1398`, `:1408`) leaves a receipt that never
// reached `DELIVERED` pending forever; here it times out too. A request Resource whose retries
// run out fails `Send`, and a stopped transport fails every receipt `LinkClosed`. Helpers come
// from the sibling `tests_parts` files.

/// Short enough to wait for, long enough that the response path cannot race it.
const OVERRIDE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Scheduling slack on top of the link check tick a timeout may land late by.
const TIMEOUT_SLACK: Duration = Duration::from_millis(500);

/// A response body that takes many parts to transfer, so a stall can hold it in `Receiving`.
const LONG_RESPONSE_LEN: usize = 512 * 1024;

/// `response()` must read `Timeout`, no earlier than the deadline and no later than one link
/// check interval plus slack after it.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_times_out(receipt: &RequestReceipt, started: Instant) {
    let outcome = timeout(OVERRIDE_TIMEOUT + INTERVAL_LINKS_CHECK + TIMEOUT_SLACK, receipt.response())
        .await
        .expect("the receipt times out");
    let elapsed = started.elapsed();
    assert_eq!(outcome, Err(RequestError::Timeout));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(elapsed >= OVERRIDE_TIMEOUT, "the receipt failed before its deadline: {elapsed:?}");
    assert!(receipt.concluded_at().is_some(), "a failed receipt has concluded");
}

/// Waits on a subscription until the receipt reports `wanted`, failing on any terminal status
/// before it, and returns when it was seen.
#[deny(clippy::await_holding_invalid_type)]
async fn await_status(
    status_rx: &mut tokio::sync::watch::Receiver<RequestStatus>,
    wanted: RequestStatus,
    bound: Duration,
) -> Instant {
    timeout(bound, async {
        loop {
            let status = *status_rx.borrow_and_update();
            if status == wanted {
                return Instant::now();
            }
            assert!(
                !matches!(status, RequestStatus::Ready | RequestStatus::Failed),
                "the receipt concluded {status:?} before reaching {wanted:?}"
            );
            status_rx.changed().await.expect("the pending entry is alive");
        }
    })
    .await
    .unwrap_or_else(|_| panic!("the receipt did not reach {wanted:?} within {bound:?}"))
}

/// Drops the Resource state of `link_id` on `transport` as the link maintenance does when a
/// link closes, which fails every transfer on it towards the retry worker's next drain.
#[deny(clippy::await_holding_invalid_type)]
async fn drop_resource_state(transport: &Transport, link_id: AddressHash) {
    let handler = transport.get_handler();
    let mut handler = handler.lock().await;
    handler.resource_manager.remove_link_state(link_id);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_default_timeout_follows_the_link_rtt() {
    let pair = loopback_request_pair("receipt-default-timeout").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let rtt = pair.handles.link.lock().await.rtt();
    assert!(rtt > Duration::ZERO, "the initiator measured its rtt on activation");
    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;

    assert_eq!(receipt.timeout(), rtt * 6 + Duration::from_millis(11_250));
    assert_eq!(receipt.status(), RequestStatus::Sent, "the default timeout is far off");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_packet_form_times_out_without_a_response() {
    let pair = loopback_request_pair("receipt-timeout-packet").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    assert_eq!(receipt.timeout(), OVERRIDE_TIMEOUT);
    assert!(receipt.started_at().is_some(), "a request packet starts its clock when sent");

    assert_times_out(&receipt, started).await;
}

/// An unregistered path answers nothing either; the receipt still times out.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_unhandled_path_times_out() {
    let pair = loopback_request_pair("receipt-timeout-no-handler").await;

    let started = Instant::now();
    let receipt = request_with(
        &pair,
        "/nobody-home",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;

    assert_times_out(&receipt, started).await;
}

/// The resource form's clock starts at `Delivered` (`RNS/Link.py:1370-1373`), not at the
/// send. The listener is stalled so the request Resource sits unprocessed for longer than the
/// override timeout: the receipt stays `Sent` with no clock running, reaches `Delivered` once
/// the listener wakes and assembles it, and only then times out, measured from there.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_resource_form_is_delivered_then_times_out() {
    let pair = loopback_request_pair("receipt-timeout-resource-form").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let body = over_mdu_body(&pair).await;

    let stall = stall_transport(&pair.handles.listener.transport).await;
    let sent = Instant::now();
    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Binary(body),
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    let mut status_rx = receipt.subscribe();
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Sent);
    assert_eq!(receipt.started_at(), None, "no clock runs before the request Resource completes");

    stall.await.expect("stall task finished");
    let delivered = await_status(&mut status_rx, RequestStatus::Delivered, LOOPBACK_TIMEOUT).await;
    assert!(
        delivered.duration_since(sent) > OVERRIDE_TIMEOUT + INTERVAL_LINKS_CHECK,
        "a clock anchored at the send would have expired the receipt while it was `Sent`"
    );
    let started = receipt.started_at().expect("delivery starts the clock");
    assert!(started.elapsed() <= delivered.elapsed() + TIMEOUT_SLACK);

    assert_times_out(&receipt, delivered).await;
}

/// `RNS/Link.py:1389`: the clock stops while a response Resource is being received. The
/// listener is stalled as soon as the connector reports `Receiving`, so the transfer sits
/// still for longer than the override timeout plus several link checks; the receipt must
/// still finish `Ready` once the listener wakes, never `Failed`. The runtime is multi-threaded
/// here so that the test task sees `Receiving` while the transfer is still under way: on one
/// thread the connector's part processing runs the whole transfer before the test task is
/// polled again, and the stall would land on a listener that has nothing left to send.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_receiving_outlasts_the_timeout_without_expiring() {
    let pair = loopback_request_pair("receipt-timeout-receiving").await;
    let body = random_payload(LONG_RESPONSE_LEN);
    let expected = body.clone();
    register_handler(&pair, "/long", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request_with(
        &pair,
        "/long",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    let mut status_rx = receipt.subscribe();
    let receiving = await_status(&mut status_rx, RequestStatus::Receiving, OVERRIDE_TIMEOUT).await;
    let stall = stall_transport(&pair.handles.listener.transport).await;
    stall.await.expect("stall task finished");

    let response = await_receipt(&receipt).await.expect("the response completes after the stall");
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(expected)));
    assert_eq!(receipt.status(), RequestStatus::Ready);
    let concluded = receipt.concluded_at().expect("a ready receipt has concluded");
    assert!(
        concluded.duration_since(receiving.into_std()) > OVERRIDE_TIMEOUT + INTERVAL_LINKS_CHECK,
        "the watchdog had the deadline behind it while the receipt was `Receiving`"
    );
}

/// A response Resource that fails in transfer fails the receipt `Receive` rather than leaving
/// it `Receiving` with no clock. The connector's own Resource state for the link is dropped
/// mid-transfer, which is what the receiver giving up or a link close does, and the retry
/// worker's drain routes the `InboundFailed` it produces to the receipt.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_failed_response_resource_fails_receive() {
    let pair = loopback_request_pair("receipt-receive").await;
    let body = random_payload(LONG_RESPONSE_LEN);
    register_handler(&pair, "/long", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let receipt = request(&pair, "/long", rmpv::Value::Nil).await;
    let mut status_rx = receipt.subscribe();
    await_status(&mut status_rx, RequestStatus::Receiving, LOOPBACK_TIMEOUT).await;
    let stall = stall_transport(&pair.handles.listener.transport).await;
    drop_resource_state(&pair.handles.connector.transport, pair.handles.link_id).await;

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Receive));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    stall.await.expect("stall task finished");
}

/// A request Resource that fails on the way out fails the receipt `Send`, through the same
/// drain: with the listener stalled the request Resource is still advertised when the
/// connector's Resource state for the link is dropped, which reports it `OutboundFailed`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_failed_request_resource_fails_send() {
    let pair = loopback_request_pair("receipt-send").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    let body = over_mdu_body(&pair).await;

    let stall = stall_transport(&pair.handles.listener.transport).await;
    let receipt = request(&pair, "/quiet", rmpv::Value::Binary(body)).await;
    assert_eq!(receipt.status(), RequestStatus::Sent);
    drop_resource_state(&pair.handles.connector.transport, pair.handles.link_id).await;

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Send));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(receipt.started_at(), None, "the clock never started");
    stall.await.expect("stall task finished");
}

/// Stopping the requester's transport leaves no watchdog to expire a receipt, so `stop()`
/// fails every pending receipt itself; `response()`, `status()` and `concluded_at()` agree.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_fails_link_closed_when_the_requester_stops() {
    let pair = loopback_request_pair("receipt-requester-stop").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;

    let receipt = request(&pair, "/quiet", rmpv::Value::Nil).await;
    let clone = receipt.clone();
    assert_eq!(receipt.status(), RequestStatus::Sent);
    timeout(LOOPBACK_TIMEOUT, pair.handles.connector.transport.stop()).await.expect("stops");

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::LinkClosed));
    assert_eq!(await_receipt(&clone).await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(receipt.concluded_at().is_some());
}
