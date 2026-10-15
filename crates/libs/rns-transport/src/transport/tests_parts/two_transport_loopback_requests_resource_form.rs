// Loopback coverage for a request that arrives as a request Resource instead of one link
// packet (`RNS/Link.py:503-510`). The listener assembles it, recomputes the request id from
// the assembled bytes (`RNS/Link.py:885-889`) and runs the registered handler as it would
// for a request packet. A response over the link MDU travels as a response Resource whose
// advertisement names the request in `q` and sets `p` (`RNS/Resource.py:1305-1318`), the
// file-response form carries raw data beside its metadata (`RNS/Link.py:839-846`), and a
// request whose advertised data size is over `max_request_size` is rejected before it is
// assembled (`RNS/Link.py:1036-1046`). Helpers come from the sibling `tests_parts` files.

/// Any request Resource whose advertised data size is over this is rejected at its
/// advertisement, before any segment is accepted.
const RESOURCE_FORM_REQUEST_LIMIT: usize = 1024;
/// Zero bytes: bz2 brings the transfer size `t` far under the limit while the data size `d`
/// stays over it, so only a gate that reads `d` rejects the request.
const OVERSIZE_COMPRESSIBLE_LEN: usize = 4 * 1024;

/// Sends `value` to `path` as a request Resource and returns the id the responder recomputes
/// from the assembled bytes together with the resource hash the connector tracks it by.
#[deny(clippy::await_holding_invalid_type)]
async fn send_request_as_resource(
    pair: &LoopbackRequestPair,
    path: &str,
    value: rmpv::Value,
) -> (RequestId, Hash) {
    let request = Link::request_payload(path, value).expect("request packs");
    let id = RequestId::new(request.resource_request_id);
    let hash = pair
        .handles
        .connector
        .transport
        .send_request_resource(
            &pair.handles.link_id,
            request.resource_request_id.to_vec(),
            request.packed,
            None,
        )
        .await
        .expect("request resource dispatched");
    (id, hash)
}

/// Waits for the response Resource answering `id` on the connector's resource events.
#[deny(clippy::await_holding_invalid_type)]
async fn await_response_resource(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    id: RequestId,
) -> ResourceComplete {
    let complete = await_resource_event(events, |event| {
        event.link_id == link_id
            && matches!(&event.kind, ResourceEventKind::Complete(complete)
                if complete.is_response
                    && complete.request_id.as_deref() == Some(id.as_bytes().as_slice()))
    })
    .await;
    complete_payload(complete)
}

/// A body one byte over the link MDU: the request cannot travel as a packet, and neither
/// can its echo.
#[deny(clippy::await_holding_invalid_type)]
async fn over_mdu_body(pair: &LoopbackRequestPair) -> Vec<u8> {
    random_payload(pair.handles.link.lock().await.link_mdu() + 1)
}

/// Like `echo_handler`, and reports the id of every request it sees on `seen`.
fn recording_echo_handler(
    seen: tokio::sync::mpsc::UnboundedSender<RequestId>,
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    move |request: InboundRequest| {
        seen.send(request.request_id()).expect("test receiver is alive");
        std::future::ready(Some(RequestResponse::Value(request.into_data())))
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_is_answered_by_the_handler() {
    let pair = loopback_request_pair("request-resource-form-round-trip").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/echo", RequestAllow::All, recording_echo_handler(seen_tx)).await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let complete = await_response_resource(&mut resource_events, link_id, id).await;
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(body));
    assert_eq!(seen_rx.recv().await, Some(id), "the handler saw the resource-form id");
}

/// `RNS/Resource.py:1305-1318`: a response Resource advertises the request id in `q` and
/// the response flag `p` (bit 4), never the request flag `u`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_oversize_response_advertises_q_and_p() {
    let mut pair = loopback_request_pair("request-resource-form-advert").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body)).await;
    await_response_resource(&mut resource_events, link_id, id).await;

    let mut response_advertisements = Vec::new();
    loop {
        match pair.connector_tap.try_recv() {
            Ok(message)
                if message.packet.destination == link_id
                    && message.packet.context == PacketContext::ResourceAdvrtisement =>
            {
                let advertisement = {
                    let link = pair.handles.link.lock().await;
                    decrypt_resource_advertisement(&link, &message.packet)
                };
                if advertisement.is_response() {
                    response_advertisements.push(advertisement);
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                panic!("interface tap lagged by {skipped} packets; the advertisement may be lost")
            }
            Err(_) => break,
        }
    }
    let [advertisement] = response_advertisements.as_slice() else {
        panic!("exactly one response advertisement reached the connector");
    };
    assert_eq!(
        advertisement.request_id.as_deref().map(Vec::as_slice),
        Some(id.as_bytes().as_slice()),
        "q names the request"
    );
    assert!(advertisement.is_response(), "p is set");
    assert!(!advertisement.is_request(), "u is clear");
    assert_ne!(advertisement.flags & 0x10, 0, "p is bit 4 of the flag byte");
}

/// `RNS/Link.py:885-889`: the responder's id is `truncated_hash(packed_request)` over the
/// assembled bytes, the same value `RNS/Link.py:504` records on the requester.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_id_is_recomputed_from_the_assembled_bytes(
) {
    let pair = loopback_request_pair("request-resource-form-id").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel();
    register_handler(&pair, "/id", RequestAllow::All, recording_echo_handler(seen_tx)).await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/id", rmpv::Value::Binary(body)).await;
    await_response_resource(&mut resource_events, link_id, id).await;

    let seen = seen_rx.recv().await.expect("handler ran once");
    log::debug!("requester id {id}, responder id {seen}");
    assert_eq!(seen, id);
    assert!(
        matches!(seen_rx.try_recv(), Err(tokio::sync::mpsc::error::TryRecvError::Empty)),
        "the handler ran exactly once"
    );
}

/// The file-response form (`RNS/Link.py:839-846`) answers a resource-form request the same
/// way it answers a packet: raw data in the resource, msgpack metadata beside it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_file_response_carries_raw_data_and_metadata(
) {
    let pair = loopback_request_pair("request-resource-form-file").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = random_payload(64);
    let expected = data.clone();
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("a.bin"))]);
    let expected_metadata = metadata.clone();
    register_handler(&pair, "/file", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Resource {
            data: data.clone(),
            metadata: Some(metadata.clone()),
        }))
    })
    .await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/file", rmpv::Value::Binary(body)).await;
    let complete = await_response_resource(&mut resource_events, link_id, id).await;
    assert_eq!(complete.data, expected, "the raw data, not an envelope");
    let packed = complete.metadata.expect("metadata rides with the resource");
    let decoded = rmpv::decode::read_value(&mut packed.as_slice()).expect("metadata is msgpack");
    assert_eq!(decoded, expected_metadata);
}

/// `RNS/Link.py:1036-1046`: a request Resource whose advertised data size is over the
/// destination's `max_request_size` is rejected at the advertisement, however small its
/// compressed transfer size, and the link goes on answering packet-form requests.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_over_max_request_size_is_rejected() {
    let mut pair = loopback_request_pair("request-resource-form-max-size").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    pair.handles
        .destination
        .lock()
        .await
        .set_max_request_size(RESOURCE_FORM_REQUEST_LIMIT)
        .expect("limit is set");
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = vec![0u8; OVERSIZE_COMPRESSIBLE_LEN];
    let (_, hash) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body)).await;
    let outcome = await_resource_event(&mut resource_events, |event| {
        event.hash == hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(outcome.kind, ResourceEventKind::OutboundRejected),
        "the oversize request is rejected, got {:?}",
        outcome.kind
    );

    let id = send_request(&pair, "/echo", rmpv::Value::from("still here")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("still here"));
}
