// Usage-pattern coverage for how long a request's send-time record solicits its response
// Resource, as a consumer who answers slowly sees it: the record of a bare
// `Link::request_packet` and of a `send_request_resource*` call lives for the default
// request timeout (`rtt * 6 + 11.25 s`, `RNS/Link.py:491`) and is pruned by the link
// watchdog. Its clock runs from the send for a request packet and from the request
// Resource's delivery for the resource form (`RNS/Link.py:1370-1373`), and stops while a
// response Resource is inbound (`:1389`); a response Resource advertised after the clock
// ran out is dropped as one nothing on the link sent (`RNS/Link.py:1046-1056`). Python's
// marker is the `RequestReceipt`, pruned only once `DELIVERED` and past its timeout
// (`:1397-1405`). A record a live `RequestReceipt` owns goes with the receipt and admits a
// response the receipt's own timeout still allows. Helpers come from the sibling
// `tests_parts` files.

/// Headroom past the record's deadline for the watchdog tick that prunes it.
const PROBE4_PRUNE_MARGIN: Duration = Duration::from_secs(2);

/// A handler that holds its response until `release` is notified, then answers `body`.
fn gated_handler(
    release: Arc<tokio::sync::Notify>,
    body: Vec<u8>,
) -> impl Fn(
    InboundRequest,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<RequestResponse>> + Send>>
       + Send
       + Sync {
    move |_: InboundRequest| {
        let release = release.clone();
        let body = body.clone();
        Box::pin(async move {
            release.notified().await;
            Some(RequestResponse::Bytes(body))
        })
    }
}

/// The default request timeout the pair's link stamps on a receipt-less record.
#[deny(clippy::await_holding_invalid_type)]
async fn default_request_timeout(pair: &LoopbackRequestPair) -> Duration {
    let rtt = pair.handles.link.lock().await.rtt();
    RequestOptions::default().timeout_for_rtt(rtt)
}

/// No response Resource for any of `ids` is assembled on `link_id` within `SIZES_SILENCE`,
/// and no inbound Resource fails there.
#[deny(clippy::await_holding_invalid_type)]
async fn probe4_assert_no_response_for(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    ids: &[RequestId],
) {
    let deadline = Instant::now() + SIZES_SILENCE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        match timeout(remaining, events.recv()).await {
            Ok(Ok(event)) if event.link_id == link_id => match &event.kind {
                ResourceEventKind::Complete(complete) => assert!(
                    !ids.iter().any(|id| {
                        complete.request_id.as_deref() == Some(id.as_bytes().as_slice())
                    }),
                    "a response whose record expired was assembled: {:?}",
                    complete.request_id
                ),
                ResourceEventKind::InboundFailed(failure) => {
                    panic!("an inbound Resource failed on the link: {failure:?}")
                }
                _ => {}
            },
            Ok(Ok(_)) => {}
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return,
        }
    }
}

/// Three receipt-less requests go out together: a bare request packet answered halfway
/// through the default request timeout is admitted, while a bare request packet and a
/// `send_request_resource` request answered past it find their records pruned and their
/// response Resources dropped without a reject; the link still answers afterwards. The
/// packet's clock runs from its send, the Resource's from its `OutboundComplete`, so the
/// wait is anchored at the later of the two and covers both.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe4_receipt_less_record_expires_on_the_default_request_timeout(
) {
    let pair = loopback_request_pair_with_mtu("sizes-probe4-record-expiry", LEGACY_MTU).await;
    let link_id = pair.handles.link_id;
    let body = over_mdu_body(&pair).await;
    let mid_gate = Arc::new(tokio::sync::Notify::new());
    let late_gate = Arc::new(tokio::sync::Notify::new());
    let legacy_gate = Arc::new(tokio::sync::Notify::new());
    register_handler(
        &pair,
        "/mid",
        RequestAllow::All,
        gated_handler(mid_gate.clone(), body.clone()),
    )
    .await;
    register_handler(
        &pair,
        "/late",
        RequestAllow::All,
        gated_handler(late_gate.clone(), body.clone()),
    )
    .await;
    register_handler(
        &pair,
        "/late-legacy",
        RequestAllow::All,
        gated_handler(legacy_gate.clone(), body.clone()),
    )
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let record_timeout = default_request_timeout(&pair).await;
    let sent_at = Instant::now();
    let mid = send_request(&pair, "/mid", rmpv::Value::Nil).await;
    let late = send_request(&pair, "/late", rmpv::Value::Nil).await;
    let (legacy, legacy_hash) =
        send_request_as_resource(&pair, "/late-legacy", rmpv::Value::Nil).await;
    await_resource_event(&mut events, |event| {
        event.hash == legacy_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
    let delivered_at = Instant::now();

    tokio::time::sleep_until(sent_at + record_timeout / 2).await;
    mid_gate.notify_one();
    let complete = await_response_resource(&mut events, link_id, mid).await;
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, mid.to_bytes(), "halfway through its record the response is admitted");
    assert_eq!(value, rmpv::Value::Binary(body.clone()));

    tokio::time::sleep_until(delivered_at + record_timeout + PROBE4_PRUNE_MARGIN).await;
    let mut events = pair.handles.connector.transport.resource_events();
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    late_gate.notify_one();
    legacy_gate.notify_one();

    probe4_assert_no_response_for(&mut events, link_id, &[late, legacy]).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a response whose record expired is dropped without a reject"
    );
    sizes_assert_link_still_answers(&pair).await;
}

/// A record a live receipt owns is not the watchdog's to prune: a `Transport::request` with
/// a timeout past the default is still `Sent` when the default would have run out, and the
/// response Resource that answers it afterwards is admitted and resolves it `Ready`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe4_receipt_owned_record_outlives_the_default_request_timeout(
) {
    let pair = loopback_request_pair_with_mtu("sizes-probe4-receipt-owned", LEGACY_MTU).await;
    let body = over_mdu_body(&pair).await;
    let gate = Arc::new(tokio::sync::Notify::new());
    register_handler(&pair, "/late", RequestAllow::All, gated_handler(gate.clone(), body.clone()))
        .await;

    let record_timeout = default_request_timeout(&pair).await;
    let receipt_timeout = record_timeout + PROBE4_PRUNE_MARGIN + RESPONSE_TIMEOUT * 2;
    let sent_at = Instant::now();
    let receipt = request_with(
        &pair,
        "/late",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(receipt_timeout),
    )
    .await;

    tokio::time::sleep_until(sent_at + record_timeout + PROBE4_PRUNE_MARGIN).await;
    assert_eq!(receipt.status(), RequestStatus::Sent, "the receipt's own timeout is in force");
    gate.notify_one();

    let response = await_receipt(&receipt).await.expect("the receipt-owned record admits it");
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(response.into_value(), Some(rmpv::Value::Binary(body)));
}

/// `RequestResponseData::into_value` is `None` for a file response on a shared handle as on
/// the last one, and the handles left behind still read the file's data and metadata.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe4_file_response_into_value_is_none_on_every_handle(
) {
    let pair = loopback_request_pair("sizes-probe4-file-into-value").await;
    let data = random_payload(64);
    let expected_data = data.clone();
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("b.bin"))]);
    let expected_metadata = metadata.clone();
    register_handler(&pair, "/file", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Resource {
            data: data.clone(),
            metadata: Some(metadata.clone()),
        }))
    })
    .await;

    let receipt = request(&pair, "/file", rmpv::Value::Nil).await;
    let response = await_receipt(&receipt).await.expect("file response arrives");
    let shared = response.clone();
    assert_eq!(shared.into_value(), None, "a shared handle to a file response has no body");
    assert_eq!(response.data(), Some(expected_data.as_slice()));
    assert_eq!(response.metadata(), Some(&expected_metadata));
    assert_eq!(response.value(), None);
    assert_eq!(response.into_value(), None, "nor does the last handle");
}
