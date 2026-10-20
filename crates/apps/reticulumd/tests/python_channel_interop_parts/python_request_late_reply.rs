// Late answers, the unanswered Resource-form
// request on the Rust responder, and PROVE_ALL next to a request on one link,
// all against Python RNS 1.5.2.
//
//   * Python -> Rust Resource-form request answered `None` by an ALLOW_ALL
//     handler: Rust accepts the request Resource on size alone (RNS/Link.py
//     :1036-1040), runs the handler exactly once and sends nothing (:844
//     twin); Python's receipt reaches DELIVERED (:1372) and then FAILED via
//     `request_timed_out` (:1398). The id the Rust handler saw equals the id
//     Python logged at send.
//   * Rust -> Python request whose Python handler answers AFTER the Rust
//     timeout (the endpoint's `delay` key; the handler runs on a daemon
//     thread, :1004-1005): the receipt fails `Timeout` first, the late reply
//     is dropped because no request is pending any more (packet form) or the
//     response advert is unsolicited (Resource form, no `Complete
//     { is_response }`), and the link still answers a following request.
//   * PROVE_ALL proves only `PacketContext::None` Data (:950/:961): a request
//     packet on a PROVE_ALL link yields no `PacketProven`, a plain Data packet
//     on the same link does.
//
// Reuses `RequestHandlerFixture` / `RequestHandlerRecord`,
// `RustRequestReceiptFixture`, `request_forms_matrix_*`,
// `request_unknown_path_*` and `rust_request_receipt_*` helpers from the
// sibling include!d files.

/// How long the Python handler holds its answer back; well past the Rust
/// request timeout below.
const LATE_REPLY_DELAY_SECS: u64 = 5;
const LATE_REPLY_RUST_TIMEOUT: Duration = Duration::from_secs(2);

fn late_reply_request_data(extra: Vec<(rmpv::Value, rmpv::Value)>) -> rmpv::Value {
    let mut entries =
        vec![(rmpv::Value::String("delay".into()), rmpv::Value::from(LATE_REPLY_DELAY_SECS))];
    entries.extend(extra);
    rmpv::Value::Map(entries)
}

/// Drains link events for `settle`, panicking on any `PacketProven` for
/// `link_id` (any hash).
#[deny(clippy::await_holding_invalid_type)]
async fn late_reply_assert_no_proof(
    events: &mut tokio::sync::broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    settle: Duration,
) {
    while let Ok(event) = tokio::time::timeout(settle, events.recv()).await {
        let event = event.expect("link event");
        if event.id != link_id {
            continue;
        }
        if let LinkEvent::PacketProven { packet_hash } = event.event {
            panic!("a request packet must not be proven, got PacketProven {packet_hash}");
        }
    }
}

/// Counts raw inbound `ResourceAdvrtisement` packets for `link_id` seen on
/// the Rust interfaces until a fixed deadline `window` from now, so a test can
/// prove a response advert really reached Rust before asserting that nothing
/// came of it. (A per-packet settle would never end: Python keeps re-sending
/// an unanswered advert and the link keepalives keep the wire busy.)
#[deny(clippy::await_holding_invalid_type)]
async fn late_reply_count_inbound_adverts(
    rx: &mut tokio::sync::broadcast::Receiver<rns_transport::iface::RxMessage>,
    link_id: AddressHash,
    window: Duration,
) -> usize {
    let mut adverts = 0;
    let deadline = tokio::time::Instant::now() + window;
    while let Ok(message) = tokio::time::timeout_at(deadline, rx.recv()).await {
        match message {
            Ok(message)
                if message.packet.destination == link_id
                    && message.packet.context == PacketContext::ResourceAdvrtisement =>
            {
                adverts += 1;
            }
            Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
    adverts
}

// ---------------------------------------------------------------------------
// Python -> Rust
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_to_rust_request_handler_none_response_leaves_the_resource_request_delivered_then_failed(
) {
    let _interop_guard = python_interop_guard().await;
    run_python_to_rust_request_handler_none_response_leaves_the_resource_request_delivered_then_failed(
    )
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_python_to_rust_request_handler_none_response_leaves_the_resource_request_delivered_then_failed(
) {
    let fixture = RequestHandlerFixture::start("python-rns-request-none-resource").await;
    let record = RequestHandlerRecord::default();
    fixture.register(record.handler(request_forms_matrix_no_reply), RequestAllow::All).await;
    let mut resource_events = fixture.transport.resource_events();

    let size = REQUEST_FORMS_MATRIX_LARGE.to_string();
    let mut client =
        fixture.connect(30.0, &["--request-size", &size, "--request-timeout", "4"]).await;
    let lines = client.json_lines().await;

    let sent = request_handler_line_with(&lines, "request_sent");
    assert_eq!(sent["request_form"], "resource", "a 48 KiB request travels as a Resource");
    let report = request_handler_line_with(&lines, "request_failed");
    assert_eq!(report["request_failed"], true);
    assert_eq!(report["request_form"], "resource");
    assert_eq!(report["status"], REQUEST_HANDLER_PY_FAILED, "DELIVERED then timed out -> FAILED");
    assert_eq!(sent["request_id"], report["request_id"]);
    assert!(
        lines.iter().all(|line| line.get("response_size").is_none()),
        "a None answer must send nothing back: {lines:?}"
    );

    // The handler ran exactly once and saw the same id Python logged.
    let (request_id, remote, data) = record.single_request();
    assert_eq!(remote, None);
    assert_eq!(
        rmpv_to_string(&data).map(|text| text.len()),
        Some(REQUEST_FORMS_MATRIX_LARGE),
        "the assembled request body reached the handler"
    );
    let rust_hex = request_id.to_string();
    let python_hex = report["request_id"].as_str().expect("python request id");
    eprintln!("request id rust={rust_hex} python={python_hex} (resource form, None answer)");
    assert_eq!(rust_hex, python_hex);

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(2)).await;
    assert_eq!(
        forms,
        vec![RequestFormsMatrixForm::InboundRequest],
        "the request Resource completed and nothing went back"
    );
}

// ---------------------------------------------------------------------------
// Rust -> Python
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_late_packet_response_after_timeout_is_ignored_and_link_serves_next(
) {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_late_packet_response_after_timeout_is_ignored_and_link_serves_next(
    )
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_late_packet_response_after_timeout_is_ignored_and_link_serves_next(
) {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-late-packet", "request-forms", &[])
            .await;
    let mut resource_events = fixture.transport.resource_events();

    let started = tokio::time::Instant::now();
    let receipt = fixture
        .request(
            late_reply_request_data(vec![]),
            RequestOptions::default().with_timeout(LATE_REPLY_RUST_TIMEOUT),
        )
        .await;

    let trail = request_unknown_path_status_trail(&receipt, Duration::from_secs(20)).await;
    let failed_after = started.elapsed();
    assert_eq!(
        request_unknown_path_statuses(&trail),
        vec![RequestStatus::Sent, RequestStatus::Failed],
        "packet form is never Delivered"
    );
    assert_eq!(receipt.response().await, Err(RequestError::Timeout));
    assert!(
        failed_after >= LATE_REPLY_RUST_TIMEOUT
            && failed_after < Duration::from_secs(LATE_REPLY_DELAY_SECS),
        "the timeout, not the late reply, concluded the receipt: {failed_after:?}"
    );

    // The Python handler answers only now; its reply lands on a link with no
    // pending request for that id.
    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "text", "the python handler did answer, late");
    sleep(Duration::from_millis(1500)).await;
    assert_eq!(receipt.status(), RequestStatus::Failed, "a late reply cannot revive a receipt");
    assert_eq!(receipt.response().await, Err(RequestError::Timeout));

    // The link still serves, and the stale reply does not leak into a new id.
    let after =
        fixture.request(rmpv::Value::String("after".into()), RequestOptions::default()).await;
    assert_ne!(after.request_id(), receipt.request_id());
    let response = rust_request_receipt_response(&after).await;
    assert_eq!(response.value().and_then(|v| v.as_str()), Some("reply:after"));
    assert_eq!(after.status(), RequestStatus::Ready);
    let report = fixture.python_report(&after).await;
    assert_eq!(report["response_kind"], "text");

    let forms =
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(1)).await;
    assert!(forms.is_empty(), "everything travelled in packet form: {forms:?}");
}

#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn rust_to_python_request_receipt_late_resource_response_after_timeout_is_dropped_unsolicited(
) {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_late_resource_response_after_timeout_is_dropped_unsolicited(
    )
    .await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_late_resource_response_after_timeout_is_dropped_unsolicited(
) {
    let mut fixture =
        RustRequestReceiptFixture::start("python-rns-request-late-resource", "request-forms", &[])
            .await;
    let mut resource_events = fixture.transport.resource_events();
    let mut iface_rx = fixture.transport.iface_rx();
    request_forms_matrix_assert_large_exceeds_mdu(&fixture).await;

    let started = tokio::time::Instant::now();
    let receipt = fixture
        .request(
            late_reply_request_data(vec![(
                rmpv::Value::String("response_size".into()),
                rmpv::Value::from(REQUEST_FORMS_MATRIX_LARGE as u64),
            )]),
            RequestOptions::default().with_timeout(LATE_REPLY_RUST_TIMEOUT),
        )
        .await;

    let outcome = tokio::time::timeout(Duration::from_secs(20), receipt.response())
        .await
        .expect("receipt concludes");
    let failed_after = started.elapsed();
    assert_eq!(
        outcome.as_ref().map(|response| response.value().map(|_| "a response body")),
        Err(&RequestError::Timeout)
    );
    assert!(
        failed_after < Duration::from_secs(LATE_REPLY_DELAY_SECS),
        "the timeout concluded the receipt before the python handler answered: {failed_after:?}"
    );

    // Python now advertises a 48 KiB response Resource for an id Rust no
    // longer awaits: the advert reaches Rust, is unsolicited, and must not be
    // received.
    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "bytes");
    assert_eq!(report["response_size"], REQUEST_FORMS_MATRIX_LARGE as u64);
    let (adverts, forms) = tokio::join!(
        late_reply_count_inbound_adverts(&mut iface_rx, fixture.link_id, Duration::from_secs(3)),
        request_forms_matrix_terminal_forms(&mut resource_events, Duration::from_secs(3))
    );
    assert!(adverts >= 1, "the late response advert never reached the Rust side");
    eprintln!("late response adverts seen on the wire: {adverts}");
    assert!(
        !forms.contains(&RequestFormsMatrixForm::InboundResponse),
        "an unsolicited response Resource must not complete: {forms:?}"
    );
    assert_eq!(receipt.status(), RequestStatus::Failed);

    // The link survives the stray advert: a fresh in-limit request is answered.
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
async fn rust_to_python_request_receipt_under_prove_all_only_the_plain_data_packet_is_proven() {
    let _interop_guard = python_interop_guard().await;
    run_rust_to_python_request_receipt_under_prove_all_only_the_plain_data_packet_is_proven().await;
}

#[deny(clippy::await_holding_invalid_type)]
async fn run_rust_to_python_request_receipt_under_prove_all_only_the_plain_data_packet_is_proven() {
    let mut fixture = RustRequestReceiptFixture::start(
        "python-rns-request-prove-all",
        "request-forms",
        &["--prove-all"],
    )
    .await;
    let mut link_events = fixture.transport.out_link_events();

    // A request packet carries PacketContext::Request: Python proves only
    // context-None Data (RNS/Link.py:950/:961), and Rust records only plain
    // Data for PacketProven, so the request round trip yields no proof event.
    let receipt =
        fixture.request(rmpv::Value::String("prove-all".into()), RequestOptions::default()).await;
    let response = rust_request_receipt_response(&receipt).await;
    assert_eq!(response.value().and_then(|v| v.as_str()), Some("reply:prove-all"));
    let report = fixture.python_report(&receipt).await;
    assert_eq!(report["response_kind"], "text");
    late_reply_assert_no_proof(&mut link_events, fixture.link_id, Duration::from_secs(2)).await;

    // ...while a plain Data packet on the very same link is proven.
    let hash = fixture.send_plain_data(b"prove me too").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = link_events.recv().await.expect("link event");
            if rust_request_receipt_is_proof_of(&event, fixture.link_id, hash) {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for the python proof of the plain data packet");
}
