// Requests for a path the responder never registered, in both directions
// against Python RNS 1.5.2.
//
// Python's `Link.handle_request` (RNS/Link.py:810) does nothing at all when
// `path_hash` is not in `destination.request_handlers` -- no reply, not even a
// log line -- and the request-dispatch worker on the Rust side mirrors that.
// The only observable on the requester is therefore "no response":
//   * packet form: the Python receipt stays SENT (never DELIVERED, so
//     `request_timed_out` cannot fire, :1398); the Rust receipt goes
//     `Sent -> Failed(Timeout)` with no `Delivered` in between.
//   * Resource form: Python accepts the request Resource on size alone as
//     long as the destination has ANY handler registered (:1036-1040; the
//     endpoint serves `/test/request`) and proves it, so the Rust receipt
//     reaches `Delivered` first and the timeout clock starts there
//     (`Sent -> Delivered -> Failed`).
// A dropped request must not take the destination with it: a second client
// asking the registered path is still answered.
//
// Reuses `RequestHandlerFixture` / `RequestHandlerRecord`,
// `RustRequestReceiptFixture`, `request_forms_matrix_terminal_forms` and the
// matrix constants from the sibling include!d files. The endpoint gained a
// `--request-path` flag for the Python -> Rust leg.

const REQUEST_UNKNOWN_PATH: &str = "/test/missing";

/// Follows a receipt's status watch until it concludes, returning every
/// status observed in order (the initial one first).
#[deny(clippy::await_holding_invalid_type)]
async fn request_unknown_path_status_trail(
    receipt: &RequestReceipt,
    budget: Duration,
) -> Vec<RequestStatus> {
    let mut status = receipt.subscribe();
    let mut trail = vec![*status.borrow_and_update()];
    let deadline = tokio::time::Instant::now() + budget;
    while !matches!(trail.last(), Some(RequestStatus::Ready | RequestStatus::Failed)) {
        match tokio::time::timeout_at(deadline, status.changed()).await {
            Ok(Ok(())) => trail.push(*status.borrow_and_update()),
            Ok(Err(_)) => {
                // Sender dropped at conclusion; the terminal value is still readable.
                let last = *status.borrow();
                if trail.last() != Some(&last) {
                    trail.push(last);
                }
                break;
            }
            Err(_) => panic!("receipt did not conclude within {budget:?}: {trail:?}"),
        }
    }
    trail
}

/// Collects every JSON line the Python endpoint prints within `settle`.
#[deny(clippy::await_holding_invalid_type)]
async fn request_unknown_path_python_lines(
    lines: &mut tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    settle: Duration,
) -> Vec<serde_json::Value> {
    let mut seen = Vec::new();
    while let Ok(Some(value)) = tokio::time::timeout(settle, lines.recv()).await {
        seen.push(value);
    }
    seen
}

// ---------------------------------------------------------------------------
// Python -> Rust
// ---------------------------------------------------------------------------

// Python asks a path Rust never registered: the handler for the registered
// path must not run, Python's packet-form receipt stays SENT, and the
// destination keeps serving -- a second client on the registered path gets
// its reply.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_unregistered_path_is_dropped_and_destination_keeps_serving()
{
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_unregistered_path_is_dropped_and_destination_keeps_serving()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_unregistered_path_is_dropped_and_destination_keeps_serving(
) {
    let fixture = RequestHandlerFixture::start("python-rns-request-unknown-path").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_handler_echo), RequestAllow::All).await;

    let mut stray = fixture.connect(5.0, &["--request-path", REQUEST_UNKNOWN_PATH]).await;
    let lines = stray.json_lines().await;

    let sent = request_handler_line_with(&lines, "request_sent");
    assert_eq!(sent["request_form"], "packet");
    let report = request_handler_line_with(&lines, "request_no_response");
    assert_eq!(report["request_no_response"], true);
    assert_eq!(report["status"], REQUEST_FORMS_MATRIX_PY_SENT, "packet form never DELIVERED");
    assert_eq!(sent["request_id"], report["request_id"]);
    assert!(
        lines.iter().all(|line| line.get("response_size").is_none()),
        "an unregistered path must not be answered: {lines:?}"
    );
    assert_eq!(record.calls.load(Ordering::SeqCst), 0, "handler ran for an unregistered path");

    // The destination is still serving the registered path.
    let mut client = fixture.connect(8.0, &[]).await;
    let lines = client.json_lines().await;
    let (request_id, _, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust", "packet");
}

// ---------------------------------------------------------------------------
// Rust -> Python
// ---------------------------------------------------------------------------

// Packet-form request for an unregistered Python path: Python says nothing
// (RNS/Link.py:810 dict miss), so the Rust receipt goes straight from Sent to
// Failed(Timeout) with no Delivered in between, and the Python endpoint never
// reports a received request.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_unregistered_path_packet_request_times_out_undelivered() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_unregistered_path_packet_request_times_out_undelivered()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_unregistered_path_packet_request_times_out_undelivered()
{
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-unknown-path-packet-out",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();

    let receipt = fixture
        .transport
        .request(
            &fixture.link_id,
            REQUEST_UNKNOWN_PATH,
            rmpv::Value::String("anyone home".into()),
            RequestOptions::default().with_timeout(Duration::from_secs(3)),
        )
        .await
        .expect("request is sent");
    eprintln!("request id rust={} (unregistered path, packet form)", receipt.request_id());

    let trail = request_unknown_path_status_trail(&receipt, Duration::from_secs(20)).await;
    assert_eq!(
        trail,
        vec![RequestStatus::Sent, RequestStatus::Failed],
        "packet form is never Delivered"
    );
    assert_eq!(receipt.response().await, Err(RequestError::Timeout));

    let python =
        request_unknown_path_python_lines(&mut fixture.python_lines, Duration::from_secs(2)).await;
    assert!(
        python.iter().all(|line| line.get("request_received").is_none()),
        "python served a request for an unregistered path: {python:?}"
    );
    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert!(forms.is_empty(), "a packet-form request moved no Resource: {forms:?}");
}

// Resource-form request for an unregistered Python path: Python accepts the
// request Resource on size alone (RNS/Link.py:1036-1040) and proves it, so the
// Rust receipt is Delivered first; the dict miss at :810 then leaves it
// unanswered and the timeout, counted from Delivered, fails it.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_unregistered_path_resource_request_is_delivered_then_times_out(
) {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_unregistered_path_resource_request_is_delivered_then_times_out()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_unregistered_path_resource_request_is_delivered_then_times_out(
) {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-unknown-path-resource-out",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();
    let mdu = { fixture.link.lock().await.link_mdu() };
    assert!(REQUEST_FORMS_MATRIX_LARGE > mdu, "probe payload must exceed the link MDU");

    let timeout = Duration::from_secs(3);
    let started = tokio::time::Instant::now();
    let receipt = fixture
        .transport
        .request(
            &fixture.link_id,
            REQUEST_UNKNOWN_PATH,
            rmpv::Value::String("x".repeat(REQUEST_FORMS_MATRIX_LARGE).into()),
            RequestOptions::default().with_timeout(timeout),
        )
        .await
        .expect("request is sent");
    eprintln!("request id rust={} (unregistered path, resource form)", receipt.request_id());

    let trail = request_unknown_path_status_trail(&receipt, Duration::from_secs(30)).await;
    assert_eq!(
        trail,
        vec![RequestStatus::Sent, RequestStatus::Delivered, RequestStatus::Failed],
        "the request Resource is accepted and proved before the timeout fails it"
    );
    assert!(
        started.elapsed() >= timeout,
        "the timeout is counted from Delivered, so at least {timeout:?} must pass: {:?}",
        started.elapsed()
    );
    assert_eq!(receipt.response().await, Err(RequestError::Timeout));

    let python =
        request_unknown_path_python_lines(&mut fixture.python_lines, Duration::from_secs(2)).await;
    assert!(
        python.iter().all(|line| line.get("request_received").is_none()),
        "python served a request for an unregistered path: {python:?}"
    );
    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::Outbound],
        "exactly one outbound request Resource completed and nothing came back"
    );
}
