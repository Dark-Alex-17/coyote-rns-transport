// Loopback coverage for the destination's `max_response_size`, the responder's own cap on
// what it sends: a packet response exactly at the cap goes out and one byte over is refused
// with `TooLarge`, nothing on the wire and one warn record; a `Resource` response is
// measured on its uncompressed payload, data plus packed metadata, with the same at-cap and
// over-cap edges; and a `Bytes` body too big for a packet is refused before it would fall
// through to a response Resource. Helpers come from the sibling `tests_parts` files.

#[deny(clippy::await_holding_invalid_type)]
async fn set_max_response_size(pair: &LoopbackRequestPair, max: usize) {
    pair.handles.destination.lock().await.set_max_response_size(max).expect("limit is set");
}

/// The size the dispatcher measures a packet-form response at: the packed
/// `[request_id, body]` frame. The id is always a 16-byte `bin`, so any id gives the length
/// the real one will.
fn packet_response_len(body: rmpv::Value) -> usize {
    ResponseFrame::new(RequestId::new([0u8; 16]), body).encode().len()
}

/// The size the dispatcher measures a `Resource` response at: the data plus the packed
/// metadata, before any compression.
fn resource_response_len(data: &[u8], metadata: &rmpv::Value) -> usize {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, metadata).expect("metadata packs");
    data.len() + packed.len()
}

/// Bounds the window with a data round trip in both directions, then checks that neither
/// a response packet nor a response Resource advertisement for `id` reached the connector
/// and that no resource event was published on the pair's link.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_nothing_sent_for(
    pair: &mut LoopbackRequestPair,
    resource_events: &mut broadcast::Receiver<ResourceEvent>,
    id: RequestId,
) {
    exchange_data_both_ways(&mut pair.handles).await;
    let summary = drain_connector_tap(pair).await;
    assert_eq!(summary.response_packets, 0, "no response packet on the wire");
    assert!(
        response_advertisements_for(&summary, id).is_empty(),
        "no response advertisement on the wire"
    );
    let link_id = pair.handles.link_id;
    loop {
        match resource_events.try_recv() {
            Ok(event) => assert!(
                event.link_id != link_id,
                "unexpected resource event on link {link_id}: {:?}",
                event.kind
            ),
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => return,
        }
    }
}

/// The one warn record the dispatcher wrote for the response it refused on `link_id`.
fn assert_one_refusal_warn(link_id: AddressHash, id: RequestId, len: usize, max: usize) {
    let refused = refused_response_records_for(link_id);
    assert_eq!(refused.len(), 1, "one record per refused response: {refused:?}");
    let (level, record) = &refused[0];
    assert_eq!(*level, log::Level::Warn, "a refused response is a warn: {record:?}");
    assert!(
        record.contains(&format!("response {id} on link {link_id}")),
        "the record names the request and the link: {record:?}"
    );
    assert!(
        record.contains(&format!("{len}B exceeds the destination response limit {max}B")),
        "the record carries the size and the cap: {record:?}"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_at_the_cap_goes_out() {
    let mut pair = loopback_request_pair("request-response-cap-packet-at").await;
    let body = rmpv::Value::from("at the cap");
    set_max_response_size(&pair, packet_response_len(body.clone())).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/capped",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Value(body.clone()))),
    )
    .await;

    let id = send_request(&pair, "/capped", rmpv::Value::Nil).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("at the cap"));
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::Packet);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_over_the_cap_is_too_large() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-packet-over").await;
    let link_id = pair.handles.link_id;
    let body = rmpv::Value::from("one over the cap");
    let len = packet_response_len(body.clone());
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/capped",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Value(body.clone()))),
    )
    .await;

    let id = send_request(&pair, "/capped", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_no_response(&mut pair).await;
    assert_one_refusal_warn(link_id, id, len, max);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_at_the_cap_goes_out() {
    let pair = loopback_request_pair("request-response-cap-resource-at").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = random_payload(64);
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("a.bin"))]);
    set_max_response_size(&pair, resource_response_len(&data, &metadata)).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/file",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: data.clone(), metadata: Some(metadata.clone()) })
        }),
    )
    .await;

    let id = send_request(&pair, "/file", rmpv::Value::Nil).await;
    let complete = await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert_eq!(complete.request_id.as_deref(), Some(id.as_bytes().as_slice()));
    assert_eq!(
        await_outcome_within(&mut outcomes, RESOURCE_TIMEOUT).await,
        ResponseOutcome::Resource(OutboundResult::Complete)
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_over_the_cap_is_too_large() {
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-resource-over").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = random_payload(64);
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("a.bin"))]);
    let len = resource_response_len(&data, &metadata);
    let max = len - 1;
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/file",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| {
            Some(RequestResponse::Resource { data: data.clone(), metadata: Some(metadata.clone()) })
        }),
    )
    .await;

    let id = send_request(&pair, "/file", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_nothing_sent_for(&mut pair, &mut resource_events, id).await;
    assert_one_refusal_warn(link_id, id, len, max);
}

/// With no cap this body travels as a response Resource (the oversize pins in the outcome
/// tests); a cap at the link MDU refuses the frame before the MDU check that would start
/// that transfer.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_oversize_bytes_over_the_cap_is_refused_before_the_resource()
{
    install_test_logger();
    let mut pair = loopback_request_pair("request-response-cap-oversize").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = over_mdu_zeros(&pair).await;
    let len = packet_response_len(rmpv::Value::Binary(body.clone()));
    let max = pair.handles.link.lock().await.link_mdu();
    assert!(len > max, "the frame outgrows the MDU");
    set_max_response_size(&pair, max).await;
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/big",
        RequestAllow::All,
        tracking_handler(outcome_tx, move |_| Some(RequestResponse::Bytes(body.clone()))),
    )
    .await;

    let id = send_request(&pair, "/big", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::TooLarge { len, max });
    assert_nothing_sent_for(&mut pair, &mut resource_events, id).await;
    assert_one_refusal_warn(link_id, id, len, max);
}
