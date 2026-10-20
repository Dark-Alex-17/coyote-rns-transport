// Request wire-form matrix against Python RNS 1.5.2.
//
// A TCP link to Python negotiates a ~16 KB link MTU (Python logs "Signalling
// link MTU of 16.38 KB", RNS/Link.py:513-514 derives `mdu` from it), so a
// request or response has to exceed that before either side uses the
// Resource form (RNS/Link.py:493 `len(packed_request) <= self.mdu`, :849
// `len(packed_response) <= self.mdu`). Every Resource-form test here uses a
// payload above the MDU and PINS the wire form through
// `Transport::resource_events()` (`Complete { is_request }`,
// `Complete { is_response }`, `OutboundComplete`), so a test named for the
// Resource form cannot silently pass in packet form.
//
// Reuses `RequestHandlerFixture` / `RequestHandlerRecord` and
// `RustRequestReceiptFixture` from the sibling include!d files.

use rns_transport::resource::ResourceEvent;

/// Above the ~16 KB TCP link MDU, well below the 64 MiB receiver cap.
const REQUEST_FORMS_MATRIX_LARGE: usize = 48 * 1024;
const REQUEST_FORMS_MATRIX_PY_SENT: u64 = 0x01;
const REQUEST_FORMS_MATRIX_PY_FAILED: u64 = 0x00;

/// Same generator as the endpoint's `request_forms_body`.
fn request_forms_matrix_body(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8).collect()
}

fn request_forms_matrix_short_reply(_data: &rmpv::Value) -> Option<RequestResponse> {
    Some(RequestResponse::Value(rmpv::Value::String("short".into())))
}

fn request_forms_matrix_no_reply(_data: &rmpv::Value) -> Option<RequestResponse> {
    None
}

fn request_forms_matrix_large_bytes(_data: &rmpv::Value) -> Option<RequestResponse> {
    Some(RequestResponse::Bytes(request_forms_matrix_body(REQUEST_FORMS_MATRIX_LARGE)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestFormsMatrixForm {
    InboundRequest,
    InboundResponse,
    Outbound,
}

fn request_forms_matrix_form_of(event: &ResourceEvent) -> Option<RequestFormsMatrixForm> {
    match &event.kind {
        ResourceEventKind::Complete(complete) if complete.is_request => {
            Some(RequestFormsMatrixForm::InboundRequest)
        }
        ResourceEventKind::Complete(complete) if complete.is_response => {
            Some(RequestFormsMatrixForm::InboundResponse)
        }
        ResourceEventKind::OutboundComplete => Some(RequestFormsMatrixForm::Outbound),
        _ => None,
    }
}

/// Drains every resource event already queued plus anything arriving within
/// `settle`, returning the terminal forms seen in order.
#[deny(clippy::await_holding_invalid_type)]
async fn request_forms_matrix_terminal_forms(
    events: &mut tokio::sync::broadcast::Receiver<ResourceEvent>,
    settle: Duration,
) -> Vec<RequestFormsMatrixForm> {
    let mut forms = Vec::new();
    while let Ok(event) = tokio::time::timeout(settle, events.recv()).await {
        match event {
            Ok(event) => forms.extend(request_forms_matrix_form_of(&event)),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                panic!("resource event stream lagged by {n}")
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
    forms
}

// ---------------------------------------------------------------------------
// Python -> Rust
// ---------------------------------------------------------------------------

// Resource-form request (RNS/Link.py:504 ids it as the truncated hash of the
// packed request, :889 recomputes it on the responder), Resource-form echo.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_resource_request_over_mdu_gets_a_resource_response() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_resource_request_over_mdu_gets_a_resource_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_resource_request_over_mdu_gets_a_resource_response() {
    let fixture = RequestHandlerFixture::start("python-rns-request-cross-res-res").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_handler_echo), RequestAllow::All).await;
    let mut resource_events = fixture.transport.resource_events();

    let size = REQUEST_FORMS_MATRIX_LARGE.to_string();
    let mut client = fixture.connect(30.0, &["--request-size", &size]).await;
    let lines = client.json_lines().await;

    let request_text = "x".repeat(REQUEST_FORMS_MATRIX_LARGE);
    let (request_id, _, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some(request_text.as_str()));
    request_handler_assert_response(
        &lines,
        request_id,
        format!("reply:{request_text}").as_bytes(),
        "resource",
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert!(
        forms.contains(&RequestFormsMatrixForm::InboundRequest),
        "the request did not arrive as a Resource: {forms:?}"
    );
    assert!(
        forms.contains(&RequestFormsMatrixForm::Outbound),
        "the response did not leave as a Resource: {forms:?}"
    );
}

// Resource-form request, short packet-form reply: the RESPONSE packet must
// carry the Resource-form id or Python's handle_response never matches it.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_resource_request_over_mdu_gets_a_packet_response() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_resource_request_over_mdu_gets_a_packet_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_resource_request_over_mdu_gets_a_packet_response() {
    let fixture = RequestHandlerFixture::start("python-rns-request-cross-res-packet").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_forms_matrix_short_reply), RequestAllow::All).await;
    let mut resource_events = fixture.transport.resource_events();

    let size = REQUEST_FORMS_MATRIX_LARGE.to_string();
    let mut client = fixture.connect(30.0, &["--request-size", &size]).await;
    let lines = client.json_lines().await;

    let (request_id, _, data) = record.single_request();
    assert_eq!(
        rmpv_to_string(&data).as_deref(),
        Some("x".repeat(REQUEST_FORMS_MATRIX_LARGE).as_str())
    );
    request_handler_assert_response(&lines, request_id, b"short", "resource");

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::InboundRequest],
        "exactly one inbound request Resource and no outbound Resource"
    );
}

// Packet-form request, Resource-form reply above the MDU.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_packet_request_gets_a_resource_response_over_mdu() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_packet_request_gets_a_resource_response_over_mdu().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_packet_request_gets_a_resource_response_over_mdu() {
    let fixture = RequestHandlerFixture::start("python-rns-request-cross-packet-res").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_forms_matrix_large_bytes), RequestAllow::All).await;
    let mut resource_events = fixture.transport.resource_events();

    let mut client = fixture.connect(30.0, &[]).await;
    let lines = client.json_lines().await;

    let (request_id, _, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(
        &lines,
        request_id,
        &request_forms_matrix_body(REQUEST_FORMS_MATRIX_LARGE),
        "packet",
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::Outbound],
        "exactly one outbound response Resource and no inbound Resource"
    );
}

// The handler returns `None`, so Rust sends nothing (the Rust twin of
// RNS/Link.py:844). The packet-form request is never DELIVERED, so Python's
// failed_callback cannot fire (:1398) and the client reports
// `request_no_response` with status SENT. The handler ran exactly once, which
// separates this from the allow-list denial where it never runs.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_none_response_leaves_the_packet_request_unanswered() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_none_response_leaves_the_packet_request_unanswered().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_none_response_leaves_the_packet_request_unanswered() {
    let fixture = RequestHandlerFixture::start("python-rns-request-cross-none").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_forms_matrix_no_reply), RequestAllow::All).await;

    let mut client = fixture.connect(5.0, &[]).await;
    let lines = client.json_lines().await;

    let sent = request_handler_line_with(&lines, "request_sent");
    let report = request_handler_line_with(&lines, "request_no_response");
    assert_eq!(report["request_no_response"], true);
    assert_eq!(
        report["status"], REQUEST_FORMS_MATRIX_PY_SENT,
        "packet form never reaches DELIVERED"
    );
    assert_eq!(sent["request_id"], report["request_id"]);
    let (request_id, _, data) = record.single_request();
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    assert_eq!(request_id.to_string(), report["request_id"].as_str().expect("python request id"));
}

// A denied Resource-form request. Python accepts an inbound request Resource
// on size alone (RNS/Link.py:1040) and so must Rust: the request Resource is
// received and proved, Python's receipt reaches DELIVERED (:1372), and with no
// response `request_timed_out` passes the `:1398` DELIVERED guard and fires
// failed_callback -- the one shape where a silent drop is observable on the
// Python side. The client request timeout is shorter than its exit deadline so
// the callback lands before it exits.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_allow_list_denied_resource_request_fails_on_python() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_allow_list_denied_resource_request_fails_on_python().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_allow_list_denied_resource_request_fails_on_python() {
    let fixture = RequestHandlerFixture::start("python-rns-request-cross-deny-resource").await;
    let record = RequestHandlerRecord::default();
    let other = to_transport_private_identity(&PrivateIdentity::new_from_rand(OsRng));
    let mut resource_events = fixture.transport.resource_events();

    let size = REQUEST_FORMS_MATRIX_LARGE.to_string();
    let mut client = fixture
        .connect(30.0, &["--identify", "--request-size", &size, "--request-timeout", "4"])
        .await;
    let identity =
        wait_for_link_identify(&mut client.in_events, client.link_id, Duration::from_secs(8)).await;
    assert_ne!(identity.address_hash, other.as_identity().address_hash);
    fixture
        .register(
            record.handler(request_forms_matrix_short_reply),
            RequestAllow::List(HashSet::from([other.as_identity().address_hash])),
        )
        .await;
    fixture.release_client().await;
    let lines = client.json_lines().await;

    let sent = request_handler_line_with(&lines, "request_sent");
    let report = request_handler_line_with(&lines, "request_failed");
    assert_eq!(report["request_failed"], true);
    assert_eq!(report["request_form"], "resource");
    assert_eq!(report["status"], REQUEST_FORMS_MATRIX_PY_FAILED);
    assert_eq!(sent["request_id"], report["request_id"]);
    assert!(
        lines.iter().all(|line| line.get("response_size").is_none()),
        "a denied request must not be answered: {lines:?}"
    );
    assert_eq!(record.calls.load(Ordering::SeqCst), 0, "handler ran for an unlisted peer");

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::InboundRequest],
        "the denied request Resource was still received and nothing went back"
    );
}

// ---------------------------------------------------------------------------
// Rust -> Python
// ---------------------------------------------------------------------------

// Resource-form request above the MDU, Resource-form echo back.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_resource_request_over_mdu_gets_a_resource_response() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_resource_request_over_mdu_gets_a_resource_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_resource_request_over_mdu_gets_a_resource_response() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-cross-res-res-out",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();
    let mdu = { fixture.link.lock().await.link_mdu() };
    eprintln!("rust link mdu={mdu}");
    assert!(REQUEST_FORMS_MATRIX_LARGE > mdu, "probe payload must exceed the link MDU");

    let request_text = "x".repeat(REQUEST_FORMS_MATRIX_LARGE);
    let receipt = fixture
        .request(rmpv::Value::String(request_text.clone().into()), RequestOptions::default())
        .await;
    let response = rust_request_receipt_response(&receipt).await;
    assert_eq!(
        response.value().and_then(|v| v.as_str()),
        Some(format!("reply:{request_text}").as_str())
    );
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "text");
    assert_eq!(report["response_size"], (REQUEST_FORMS_MATRIX_LARGE + 6) as u64);

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert!(
        forms.contains(&RequestFormsMatrixForm::Outbound),
        "the request did not leave as a Resource: {forms:?}"
    );
    assert!(
        forms.contains(&RequestFormsMatrixForm::InboundResponse),
        "the response did not arrive as a Resource: {forms:?}"
    );
}

// Resource-form request above the MDU, 16-byte packet-form reply carrying the
// Resource-form id.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_resource_request_over_mdu_gets_a_packet_response() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_resource_request_over_mdu_gets_a_packet_response().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_resource_request_over_mdu_gets_a_packet_response() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-cross-res-packet-out",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();

    let data = rmpv::Value::Map(vec![
        (rmpv::Value::String("response_size".into()), rmpv::Value::from(16)),
        (
            rmpv::Value::String("pad".into()),
            rmpv::Value::String("x".repeat(REQUEST_FORMS_MATRIX_LARGE).into()),
        ),
    ]);
    let receipt = fixture.request(data, RequestOptions::default()).await;
    let response = rust_request_receipt_response(&receipt).await;
    let body = match response.value() {
        Some(rmpv::Value::Binary(body)) => body.clone(),
        other => panic!("expected a binary response, got {other:?}"),
    };
    assert_eq!(body, request_forms_matrix_body(16));
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "bytes");
    assert_eq!(report["response_size"], 16);
    assert_eq!(report["response_sha256"], digest_hex(&body));

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::Outbound],
        "exactly one outbound request Resource and no inbound Resource"
    );
}

// Packet-form request, Resource-form reply above the MDU.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_packet_request_gets_a_resource_response_over_mdu() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_packet_request_gets_a_resource_response_over_mdu().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_packet_request_gets_a_resource_response_over_mdu() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-cross-packet-res-out",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();

    let data = rmpv::Value::Map(vec![(
        rmpv::Value::String("response_size".into()),
        rmpv::Value::from(REQUEST_FORMS_MATRIX_LARGE as u64),
    )]);
    let receipt = fixture.request(data, RequestOptions::default()).await;
    let response = rust_request_receipt_response(&receipt).await;
    let body = match response.value() {
        Some(rmpv::Value::Binary(body)) => body.clone(),
        other => panic!("expected a binary response, got {other:?}"),
    };
    assert_eq!(body, request_forms_matrix_body(REQUEST_FORMS_MATRIX_LARGE));
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "bytes");
    assert_eq!(report["response_size"], REQUEST_FORMS_MATRIX_LARGE as u64);
    assert_eq!(report["response_sha256"], digest_hex(&body));

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::InboundResponse],
        "exactly one inbound response Resource and no outbound Resource"
    );
}

// Two packet-form requests in flight on the same link at once. Each receipt
// must resolve to its own reply and each Python report must name the matching
// Rust id -- one shared link, two distinct request ids.
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_two_concurrent_requests_resolve_separately() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_two_concurrent_requests_resolve_separately().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_two_concurrent_requests_resolve_separately() {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-cross-two", "request-forms", &[])
            .await;

    let first =
        fixture.request(rmpv::Value::String("first".into()), RequestOptions::default()).await;
    let second =
        fixture.request(rmpv::Value::String("second".into()), RequestOptions::default()).await;
    assert_ne!(first.request_id(), second.request_id(), "distinct requests get distinct ids");

    let (first_response, second_response) =
        tokio::join!(rust_request_receipt_response(&first), rust_request_receipt_response(&second));
    assert_eq!(first_response.value().and_then(|v| v.as_str()), Some("reply:first"));
    assert_eq!(second_response.value().and_then(|v| v.as_str()), Some("reply:second"));

    let mut reports = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), async {
        while reports.len() < 2 {
            let value = fixture.python_lines.recv().await.expect("python endpoint stdout closed");
            if value.get("request_received").is_some() {
                reports.push(value);
            }
        }
    })
    .await
    .expect("timed out waiting for both python request reports");

    for (receipt, size) in [(&first, 11u64), (&second, 12u64)] {
        let rust_hex = receipt.request_id().to_string();
        let report = reports
            .iter()
            .find(|report| report["request_id"].as_str() == Some(rust_hex.as_str()))
            .unwrap_or_else(|| panic!("no python report for request {rust_hex}: {reports:?}"));
        eprintln!("request id rust={rust_hex} python={}", report["request_id"]);
        assert_eq!(report["response_kind"], "text");
        assert_eq!(report["response_size"], size);
    }
}
