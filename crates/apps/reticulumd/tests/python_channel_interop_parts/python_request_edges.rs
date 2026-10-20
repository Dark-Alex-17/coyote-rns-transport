// The close path, the envelope types and the file response against Python
// RNS 1.5.2, plus the packet-form request over `max_request_size`.
//
// README "Sending requests" promises pinned here with Python in the loop:
//   * "A link that closes, from either side, fails every pending receipt with
//     RequestError::LinkClosed": Python holds its answer and tears the link
//     down (`Link.teardown`, LINKCLOSE packet) while the Rust request waits,
//     so the failure must come from the peer's close well ahead of the
//     receipt timeout.
//   * "every other response is the envelope body, read through value()": a
//     Python dict reply arrives as a `Value::Map` with umsgpack's type
//     mapping and key order intact, as a packet and as a Resource, and the
//     received map re-packs to the exact bytes Python packed.
//   * "A response Resource with metadata is a file response, read through
//     data() and metadata()": Python always serves a file reply as a Resource
//     (RNS/Link.py:835-845), even one far below the link MDU.
// README "Request handlers": "a request packet over the limit is dropped
// silently" -- the packet-form twin of the Resource size-limit cell.
//
// Reuses `RustRequestReceiptFixture`, `RequestHandlerFixture` /
// `RequestHandlerRecord`, the matrix helpers and the unknown-path status
// trail from the sibling include!d files.

const REQUEST_EDGES_TEARDOWN_AFTER_SECS: u64 = 1;
const REQUEST_EDGES_FILE_SIZE: usize = 64;
const REQUEST_EDGES_FILE_META: &str = "python-file-meta";
/// Below the 16 KiB link MDU (packet form) and above the handler's limit.
const REQUEST_EDGES_PACKET_OVER_LIMIT: usize = 8000;

fn request_edges_str(text: &str) -> rmpv::Value {
    rmpv::Value::String(text.into())
}

fn request_edges_map(entries: Vec<(&str, rmpv::Value)>) -> rmpv::Value {
    rmpv::Value::Map(entries.into_iter().map(|(k, v)| (request_edges_str(k), v)).collect())
}

/// The dict `serve_request` returns for `response: "map"`, in its insertion
/// order, as umsgpack packs it: str, int, bool, list, bytes.
fn request_edges_expected_map(echo: &str, pad_size: usize) -> rmpv::Value {
    request_edges_map(vec![
        ("echo", request_edges_str(echo)),
        ("count", rmpv::Value::from(7u64)),
        ("ok", rmpv::Value::Boolean(true)),
        (
            "items",
            rmpv::Value::Array(vec![
                rmpv::Value::from(1u64),
                rmpv::Value::from(2u64),
                request_edges_str("three"),
            ]),
        ),
        ("raw", rmpv::Value::Binary(vec![0, 1, 2])),
        ("pad", rmpv::Value::Binary(request_forms_matrix_body(pad_size))),
    ])
}

fn request_edges_packed_digest(value: &rmpv::Value) -> String {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, value).expect("value packs");
    digest_hex(&packed)
}

/// Compares two maps through a summary so a failure never prints a 48 KiB
/// member.
fn request_edges_assert_map_eq(actual: Option<&rmpv::Value>, expected: &rmpv::Value) {
    let actual = actual.expect("an envelope body, not a file response");
    let (Some(actual_entries), Some(expected_entries)) = (actual.as_map(), expected.as_map())
    else {
        panic!("expected a map, got {actual:?}");
    };
    assert_eq!(actual_entries.len(), expected_entries.len(), "map length");
    for ((ak, av), (ek, ev)) in actual_entries.iter().zip(expected_entries) {
        assert_eq!(ak, ek, "map key order");
        if ak.as_str() == Some("pad") {
            assert_eq!(av.as_slice().map(<[u8]>::len), ev.as_slice().map(<[u8]>::len), "pad len");
            assert_eq!(av.as_slice().map(digest_hex), ev.as_slice().map(digest_hex), "pad digest");
        } else {
            assert_eq!(av, ev, "value of {ak}");
        }
    }
}

/// Waits for `LinkEvent::Closed` on `link_id`.
#[deny(clippy::await_holding_invalid_type)]
async fn request_edges_await_closed(
    events: &mut tokio::sync::broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    budget: Duration,
) {
    tokio::time::timeout(budget, async {
        loop {
            match events.recv().await {
                Ok(event) if event.id == link_id && matches!(event.event, LinkEvent::Closed) => {
                    return;
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    panic!("link event stream lagged by {n}")
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    panic!("link event stream closed")
                }
            }
        }
    })
    .await
    .expect("the python teardown must close the rust link");
}

// ---------------------------------------------------------------------------
// Rust -> Python: Python closes the link under a pending request
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_python_teardown_fails_the_pending_request_link_closed() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_python_teardown_fails_the_pending_request_link_closed()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_python_teardown_fails_the_pending_request_link_closed()
{
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-edges-teardown", "request-forms", &[])
            .await;
    let mut link_events = fixture.transport.out_link_events();

    let timeout = Duration::from_secs(30);
    let data = request_edges_map(vec![
        ("response", request_edges_str("none")),
        ("teardown_after", rmpv::Value::from(REQUEST_EDGES_TEARDOWN_AFTER_SECS)),
    ]);
    let started = tokio::time::Instant::now();
    let receipt = fixture.request(data, RequestOptions::default().with_timeout(timeout)).await;

    let (trail, report) = tokio::join!(
        request_unknown_path_status_trail(&receipt, Duration::from_secs(15)),
        fixture.python_report(&receipt)
    );
    let elapsed = started.elapsed();
    assert_eq!(report["response_kind"], "none", "python held the answer back");
    assert_eq!(
        request_unknown_path_statuses(&trail),
        vec![RequestStatus::Sent, RequestStatus::Failed],
        "a packet-form request is never Delivered"
    );
    assert_eq!(receipt.response().await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(
        elapsed < Duration::from_secs(10),
        "LinkClosed must come from the peer's close, not the {timeout:?} timeout; took {elapsed:?}"
    );
    eprintln!("request id rust={} failed LinkClosed after {elapsed:?}", receipt.request_id());

    request_edges_await_closed(&mut link_events, fixture.link_id, Duration::from_secs(5)).await;
    let torn_down =
        request_unknown_path_python_lines(&mut fixture.python_lines, Duration::from_secs(3))
            .await
            .into_iter()
            .find(|line| line.get("link_torn_down").is_some())
            .expect("python reports the teardown");
    assert_eq!(torn_down["link_torn_down"], true);
}

// ---------------------------------------------------------------------------
// Rust -> Python: dict reply as a packet and as a Resource
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_dict_reply_packet_is_read_as_a_map() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_dict_reply_packet_is_read_as_a_map().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_dict_reply_packet_is_read_as_a_map() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-edges-map-packet",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();

    let data = request_edges_map(vec![
        ("response", request_edges_str("map")),
        ("echo", request_edges_str("ping")),
        ("pad_size", rmpv::Value::from(0u64)),
    ]);
    let receipt = fixture.request(data, RequestOptions::default()).await;
    let response = rust_request_receipt_response(&receipt).await;
    let expected = request_edges_expected_map("ping", 0);
    request_edges_assert_map_eq(response.value(), &expected);
    assert_eq!(response.data(), None, "a dict reply is not a file response");
    assert_eq!(response.metadata(), None);
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "map");
    assert_eq!(
        report["response_sha256"],
        request_edges_packed_digest(response.value().expect("map")),
        "the received map re-packs to the bytes python packed"
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert!(forms.is_empty(), "a small dict reply travels as a packet: {forms:?}");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_dict_reply_resource_is_read_as_a_map() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_dict_reply_resource_is_read_as_a_map().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_dict_reply_resource_is_read_as_a_map() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-edges-map-resource",
        "request-forms",
        &[],
    )
    .await;
    let mut resource_events = fixture.transport.resource_events();
    request_forms_matrix_assert_large_exceeds_mdu(&fixture).await;

    let data = request_edges_map(vec![
        ("response", request_edges_str("map")),
        ("echo", request_edges_str("pong")),
        ("pad_size", rmpv::Value::from(REQUEST_FORMS_MATRIX_LARGE as u64)),
    ]);
    let receipt = fixture.request(data, RequestOptions::default()).await;
    let (trail, report) = tokio::join!(
        request_unknown_path_status_trail(&receipt, Duration::from_secs(20)),
        fixture.python_report(&receipt)
    );
    assert_eq!(report["response_kind"], "map");
    assert_eq!(
        request_unknown_path_statuses(&trail),
        vec![RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready],
        "a dict reply over the MDU comes back as a response Resource"
    );
    let response = rust_request_receipt_response(&receipt).await;
    let expected = request_edges_expected_map("pong", REQUEST_FORMS_MATRIX_LARGE);
    request_edges_assert_map_eq(response.value(), &expected);
    assert_eq!(response.data(), None, "an envelope Resource is not a file response");
    assert_eq!(
        report["response_sha256"],
        request_edges_packed_digest(response.value().expect("map")),
        "the received map re-packs to the bytes python packed"
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert_eq!(forms, vec![RequestFormsMatrixForm::InboundResponse]);
}

// ---------------------------------------------------------------------------
// Rust -> Python: file response with metadata
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_file_reply_is_read_through_data_and_metadata() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_file_reply_is_read_through_data_and_metadata().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_file_reply_is_read_through_data_and_metadata() {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-edges-file", "request-forms", &[])
            .await;
    let mut resource_events = fixture.transport.resource_events();
    let mdu = { fixture.link.lock().await.link_mdu() };
    assert!(REQUEST_EDGES_FILE_SIZE < mdu, "the file fits a packet; python still sends a Resource");

    let data = request_edges_map(vec![
        ("response", request_edges_str("file")),
        ("file_size", rmpv::Value::from(REQUEST_EDGES_FILE_SIZE as u64)),
    ]);
    let receipt = fixture.request(data, RequestOptions::default()).await;
    let (trail, report) = tokio::join!(
        request_unknown_path_status_trail(&receipt, Duration::from_secs(20)),
        fixture.python_report(&receipt)
    );
    assert_eq!(report["response_kind"], "file");
    assert_eq!(report["response_size"], REQUEST_EDGES_FILE_SIZE as u64);
    assert_eq!(
        request_unknown_path_statuses(&trail),
        vec![RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready],
        "a file reply is always a response Resource"
    );

    let response = rust_request_receipt_response(&receipt).await;
    let body = request_forms_matrix_body(REQUEST_EDGES_FILE_SIZE);
    assert_eq!(response.data(), Some(body.as_slice()), "file bytes");
    assert_eq!(report["response_sha256"], digest_hex(&body));
    assert_eq!(response.metadata(), Some(&request_edges_str(REQUEST_EDGES_FILE_META)));
    assert_eq!(response.value(), None, "a file response has no envelope body");
    assert_eq!(response.clone().into_value(), None);
    assert_eq!(receipt.status(), RequestStatus::Ready);

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert_eq!(forms, vec![RequestFormsMatrixForm::InboundResponse]);
}

// ---------------------------------------------------------------------------
// Python -> Rust: packet-form request over max_request_size
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_packet_request_over_max_request_size_is_dropped_silently() {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_packet_request_over_max_request_size_is_dropped_silently()
        .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_packet_request_over_max_request_size_is_dropped_silently(
) {
    let fixture = RequestHandlerFixture::start("python-rns-request-edges-packet-limit").await;
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

    let size = REQUEST_EDGES_PACKET_OVER_LIMIT.to_string();
    let mut client = fixture.connect(6.0, &["--request-size", &size]).await;
    let lines = client.json_lines().await;

    let sent = request_handler_line_with(&lines, "request_sent");
    assert_eq!(sent["request_form"], "packet", "8000 B stays under the 16 KiB link MDU");
    let report = request_handler_line_with(&lines, "request_no_response");
    assert_eq!(report["status"], REQUEST_HANDLER_PY_SENT, "python receipt stays SENT");
    assert_eq!(sent["request_id"], report["request_id"]);
    assert!(
        lines.iter().all(|line| line.get("response_size").is_none()),
        "an over-limit request packet must not be answered: {lines:?}"
    );
    assert_eq!(record.calls.load(Ordering::SeqCst), 0, "handler ran for an over-limit packet");
    eprintln!(
        "request id python={} (over max_request_size, packet form, dropped)",
        report["request_id"]
    );

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert!(forms.is_empty(), "no Resource is involved in a dropped request packet: {forms:?}");

    // The destination keeps serving an in-limit request from a fresh client.
    let mut second = fixture.connect(8.0, &[]).await;
    let lines = second.json_lines().await;
    let (request_id, remote, data) = record.single_request();
    assert_eq!(remote, None);
    assert_eq!(rmpv_to_string(&data).as_deref(), Some("hello-rust"));
    request_handler_assert_response(&lines, request_id, b"reply:hello-rust", "packet");
}
