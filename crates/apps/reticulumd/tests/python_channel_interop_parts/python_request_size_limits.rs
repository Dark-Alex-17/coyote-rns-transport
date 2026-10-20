// Size policy and the Resource-form `None` answer against Python RNS 1.5.2.
//
// The matrix pins the happy wire forms; these cells pin what a consumer sees
// when one side's size policy refuses the other side's Resource:
//   * Python -> Rust: `SingleInputDestination::set_max_request_size` below a
//     Resource-form Python request. Rust refuses the advert (RESOURCE_RCL),
//     so the Python sender's Resource ends REJECTED and
//     `request_resource_concluded` (RNS/Link.py:1367-1385) fails the receipt
//     at once through `failed_callback` -- no timeout wait. The handler never
//     runs and the destination keeps serving an in-limit request.
//   * Rust -> Python: `RequestOptions::with_max_response_size` below the
//     Python handler's Resource-form reply. The receipt fails `Oversize`
//     (advert `d` gate) rather than waiting for the timeout, Python's
//     `Resource.reject` path is benign (:160), and the link stays usable for
//     a following in-limit request.
//   * Rust -> Python Resource-form request answered with `None`
//     (RNS/Link.py:844 sends nothing): the receipt reaches `Delivered` first
//     (:1372 twin) and only then fails `Timeout`, with the Python handler
//     having run once.
//
// Reuses `RequestHandlerFixture` / `RequestHandlerRecord`,
// `RustRequestReceiptFixture`, `request_forms_matrix_terminal_forms`,
// `request_unknown_path_status_trail` and the matrix constants from the
// sibling include!d files.

/// Well below a 48 KiB Resource and above any packet-form request or reply
/// these cells send.
const REQUEST_SIZE_LIMITS_SMALL: usize = 1024;

// ---------------------------------------------------------------------------
// Python -> Rust
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_resource_request_over_max_request_size_is_rejected_promptly(
) {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_resource_request_over_max_request_size_is_rejected_promptly(
    )
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_resource_request_over_max_request_size_is_rejected_promptly(
) {
    let fixture = RequestHandlerFixture::start("python-rns-request-size-limit-in").await;
    let record = RequestHandlerRecord::default();
    {
        fixture
            .destination
            .lock()
            .await
            .set_max_request_size(REQUEST_SIZE_LIMITS_SMALL)
            .expect("request size limit set");
    }
    fixture.register(record.handler(request_handler_echo), RequestAllow::All).await;
    let mut resource_events = fixture.transport.resource_events();

    // The client's own request timeout is long enough that a receipt failed
    // through the timeout path could not land inside the wall-clock bound
    // asserted below; only a prompt rejection can.
    let size = REQUEST_FORMS_MATRIX_LARGE.to_string();
    let started = tokio::time::Instant::now();
    let mut client =
        fixture.connect(40.0, &["--request-size", &size, "--request-timeout", "25"]).await;
    let lines = client.json_lines().await;
    let elapsed = started.elapsed();

    let sent = request_handler_line_with(&lines, "request_sent");
    assert_eq!(sent["request_form"], "resource", "a 48 KiB request travels as a Resource");
    let report = request_handler_line_with(&lines, "request_failed");
    assert_eq!(report["request_failed"], true);
    assert_eq!(report["status"], REQUEST_HANDLER_PY_FAILED, "python receipt FAILED");
    assert_eq!(sent["request_id"], report["request_id"]);
    assert!(
        lines.iter().all(|line| line.get("response_size").is_none()),
        "an over-limit request must not be answered: {lines:?}"
    );
    assert_eq!(record.calls.load(Ordering::SeqCst), 0, "handler ran for an over-limit request");
    assert!(
        elapsed < Duration::from_secs(20),
        "rejection must conclude the python receipt before its 25 s request timeout, took {elapsed:?}"
    );
    eprintln!(
        "request id python={} (over max_request_size, resource form, failed after {elapsed:?})",
        report["request_id"]
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert!(
        !forms.contains(&RequestFormsMatrixForm::InboundRequest),
        "the over-limit request Resource must not complete: {forms:?}"
    );
    assert!(
        !forms.contains(&RequestFormsMatrixForm::Outbound),
        "nothing may go back for a rejected request: {forms:?}"
    );

    // The destination keeps serving: an in-limit packet-form request from a
    // fresh client is answered as usual.
    let mut second = fixture.connect(8.0, &[]).await;
    let lines = second.json_lines().await;
    let (request_id, remote, data) = record.single_request();
    assert_eq!(remote, None);
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust", "packet");
}

// ---------------------------------------------------------------------------
// Rust -> Python
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_resource_response_over_max_response_size_fails_oversize() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_resource_response_over_max_response_size_fails_oversize()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_resource_response_over_max_response_size_fails_oversize(
) {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-size-limit-out", "request-forms", &[])
            .await;
    let mut resource_events = fixture.transport.resource_events();
    request_forms_matrix_assert_large_exceeds_mdu(&fixture).await;

    let data = rmpv::Value::Map(vec![(
        rmpv::Value::String("response_size".into()),
        rmpv::Value::from(REQUEST_FORMS_MATRIX_LARGE as u64),
    )]);
    let receipt = fixture
        .request(
            data,
            RequestOptions::default()
                .with_max_response_size(REQUEST_SIZE_LIMITS_SMALL)
                .with_timeout(Duration::from_secs(15)),
        )
        .await;

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "bytes");
    assert_eq!(report["response_size"], REQUEST_FORMS_MATRIX_LARGE as u64);

    let started = tokio::time::Instant::now();
    let outcome = tokio::time::timeout(Duration::from_secs(20), receipt.response())
        .await
        .expect("receipt concludes");
    let elapsed = started.elapsed();
    // Compared through a summary so a failure does not print a 48 KiB body.
    assert_eq!(
        outcome.as_ref().map(|response| response.value().map(|_| "a response body")),
        Err(&RequestError::Oversize),
        "advert larger than max_response_size"
    );
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(
        elapsed < Duration::from_secs(10),
        "Oversize must come from the advert gate, not the 15 s timeout; took {elapsed:?}"
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert!(
        !forms.contains(&RequestFormsMatrixForm::InboundResponse),
        "the over-limit response Resource must not complete: {forms:?}"
    );

    // The link survives the rejection: a plain in-limit request is answered.
    let after =
        fixture.request(rmpv::Value::String("after".into()), RequestOptions::default()).await;
    let response = rust_request_receipt_response(&after).await;
    assert_eq!(response.value().and_then(|v| v.as_str()), Some("reply:after"));
    assert_eq!(after.status(), RequestStatus::Ready);
    let report = fixture.python_report(&after).await;
    assert_eq!(report["response_kind"], "text");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_resource_request_none_response_is_delivered_then_times_out()
{
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_resource_request_none_response_is_delivered_then_times_out()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_resource_request_none_response_is_delivered_then_times_out(
) {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-size-limit-none",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();
    request_forms_matrix_assert_large_exceeds_mdu(&fixture).await;

    let timeout = Duration::from_secs(3);
    let data = rmpv::Value::Map(vec![
        (rmpv::Value::String("response".into()), rmpv::Value::String("none".into())),
        (
            rmpv::Value::String("pad".into()),
            rmpv::Value::String("x".repeat(REQUEST_FORMS_MATRIX_LARGE).into()),
        ),
    ]);
    let receipt = fixture.request(data, RequestOptions::default().with_timeout(timeout)).await;

    // Subscribe before the Resource can conclude so `Sent` is still the
    // current status when the trail starts.
    let (trail, report) = tokio::join!(
        request_unknown_path_status_trail(&receipt, Duration::from_secs(30)),
        fixture.python_report(&receipt)
    );
    assert_eq!(report["response_kind"], "none", "the python handler ran and answered None");
    assert_eq!(
        request_unknown_path_statuses(&trail),
        vec![RequestStatus::Sent, RequestStatus::Delivered, RequestStatus::Failed],
        "a request Resource is proved before the timeout clock starts"
    );
    assert_eq!(receipt.response().await, Err(RequestError::Timeout));
    let delivered_at = request_unknown_path_instant_of(&trail, RequestStatus::Delivered);
    let failed_at = request_unknown_path_instant_of(&trail, RequestStatus::Failed);
    assert!(
        failed_at.duration_since(delivered_at) >= timeout,
        "the request clock runs from delivery: {:?} between Delivered and Failed, timeout {timeout:?}",
        failed_at.duration_since(delivered_at)
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::Outbound],
        "exactly one outbound request Resource and nothing inbound"
    );
}
