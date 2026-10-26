// Loopback coverage for the edges of the resource-form request path. The listener still
// posts `ResourceEventKind::Complete(ResourceComplete { is_request: true, .. })` for every
// assembled request Resource it answers or drops; the `max_request_size` gate reads the data size
// `d` inclusively (`RNS/Link.py:1039`) and a rejected link goes on carrying request
// Resources; a `Resource { metadata: None }` response is raw data, not an envelope
// (`RNS/Link.py:839-846`); the response Resource honours the registration's
// `auto_compress` (`RNS/Link.py:846`, `:852`); an unregistered path, an allow list that
// denies the unidentified peer (`RNS/Link.py:818-824`) and a body that is not a request
// frame all stay silent while the link keeps answering. Helpers come from the sibling
// `tests_parts` files.

/// Zero bytes one past the link MDU: the `[request_id, body]` envelope cannot travel as a
/// packet, and the body compresses to a few bytes under bz2, so the compressed flag `c`
/// alone tells whether `auto_compress` was honoured.
#[deny(clippy::await_holding_invalid_type)]
async fn over_mdu_zeros(pair: &LoopbackRequestPair) -> Vec<u8> {
    vec![0u8; pair.handles.link.lock().await.link_mdu() + 1]
}

/// What the connector's interface tap saw for the pair's link since the last drain: how many
/// Response packets, and every response Resource advertisement, decrypted.
struct ConnectorTapSummary {
    response_packets: usize,
    response_advertisements: Vec<ResourceAdvertisement>,
}

/// Drains the connector tap. A lagged tap would make the counts meaningless, so it fails
/// the test instead.
#[deny(clippy::await_holding_invalid_type)]
async fn drain_connector_tap(pair: &mut LoopbackRequestPair) -> ConnectorTapSummary {
    let link_id = pair.handles.link_id;
    let mut summary =
        ConnectorTapSummary { response_packets: 0, response_advertisements: Vec::new() };
    loop {
        match pair.connector_tap.try_recv() {
            Ok(message) if message.packet.destination == link_id => {
                if message.packet.context == PacketContext::Response {
                    summary.response_packets += 1;
                } else if message.packet.context == PacketContext::ResourceAdvrtisement {
                    let advertisement = {
                        let link = pair.handles.link.lock().await;
                        decrypt_resource_advertisement(&link, &message.packet)
                    };
                    if advertisement.is_response() {
                        summary.response_advertisements.push(advertisement);
                    }
                }
            }
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                panic!("interface tap lagged by {skipped} packets; the summary is unreliable")
            }
            Err(_) => return summary,
        }
    }
}

/// The response advertisements in `summary` whose `q` names `id`.
fn response_advertisements_for(
    summary: &ConnectorTapSummary,
    id: RequestId,
) -> Vec<&ResourceAdvertisement> {
    summary
        .response_advertisements
        .iter()
        .filter(|advertisement| {
            advertisement.request_id.as_deref().map(Vec::as_slice) == Some(id.as_bytes().as_slice())
        })
        .collect()
}

/// Sends already packed request bytes as a request Resource under `id`, the way a requester
/// that computed `truncated_hash(packed)` itself would (`RNS/Link.py:504-506`), and returns
/// the resource hash the connector tracks it by.
#[deny(clippy::await_holding_invalid_type)]
async fn send_packed_request_as_resource(
    pair: &LoopbackRequestPair,
    id: RequestId,
    packed: Vec<u8>,
) -> Hash {
    pair.handles
        .connector
        .transport
        .send_request_resource(&pair.handles.link_id, id.to_bytes().to_vec(), packed, None)
        .await
        .expect("request resource dispatched")
}

/// `register_handler` with an explicit `auto_compress`.
#[deny(clippy::await_holding_invalid_type)]
async fn register_handler_with_compression(
    pair: &LoopbackRequestPair,
    path: &str,
    auto_compress: bool,
    handler: impl RequestHandler + 'static,
) {
    pair.handles
        .destination
        .lock()
        .await
        .register_request_handler(path, Arc::new(handler), RequestAllow::All, auto_compress)
        .expect("handler registers");
}

/// Waits for the listener's `Complete(ResourceComplete { is_request: true, .. })` for the
/// pair's link.
#[deny(clippy::await_holding_invalid_type)]
async fn await_request_resource_complete(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
) -> ResourceComplete {
    let complete = await_resource_event(events, |event| {
        event.link_id == link_id
            && matches!(&event.kind, ResourceEventKind::Complete(complete) if complete.is_request)
    })
    .await;
    complete_payload(complete)
}

/// The `Complete(ResourceComplete { is_request: true, .. })` broadcast is the resource-form
/// twin of `LinkEvent::Data(Request)`: it posts for a request the registered handler answers,
/// carries the assembled bytes (which hash back to the requester's id) and names the request
/// in `q`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_still_posts_complete_with_is_request() {
    let pair = loopback_request_pair("request-resource-form-complete-event").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;

    let complete = await_request_resource_complete(&mut listener_events, link_id).await;
    assert!(!complete.is_response, "a request, not a response");
    assert_eq!(
        complete.request_id.as_deref(),
        Some(id.as_bytes().as_slice()),
        "q names the request"
    );
    assert_eq!(
        RequestId::of_packed_request(&complete.data),
        id,
        "the event carries the assembled request bytes"
    );

    let response = await_response_resource(&mut connector_events, link_id, id).await;
    let (_, echoed) = unpack_response_envelope(&response.data).expect("response envelope");
    assert_eq!(echoed, rmpv::Value::Binary(body), "the handler still answered it");
}

/// `read_size(packet) <= self.destination.max_request_size` (`RNS/Link.py:1039`): a limit
/// one byte under the packed request rejects its Resource at the advertisement, before the
/// handler runs; a limit equal to it admits the same bytes, and that second request Resource
/// travels over the link the rejection just used.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_max_request_size_is_inclusive_of_d() {
    let pair = loopback_request_pair("request-resource-form-size-boundary").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone())).await;

    let body = over_mdu_body(&pair).await;
    let request = Link::request_payload("/echo", rmpv::Value::Binary(body.clone())).expect("packs");
    let id = RequestId::new(request.resource_request_id);
    let d = request.packed.len();

    pair.handles.destination.lock().await.set_max_request_size(d - 1).expect("limit is set");
    let hash = send_packed_request_as_resource(&pair, id, request.packed.clone()).await;
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(
        matches!(outcome, ResourceEventKind::OutboundRejected { .. }),
        "d == limit + 1 is rejected, got {outcome:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "rejected before any handler ran");

    pair.handles.destination.lock().await.set_max_request_size(d).expect("limit is set");
    let hash = send_packed_request_as_resource(&pair, id, request.packed).await;
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(
        matches!(outcome, ResourceEventKind::OutboundComplete),
        "d == limit is admitted on the requester, got {outcome:?}"
    );
    let response = await_response_resource(&mut connector_events, link_id, id).await;
    let (_, echoed) = unpack_response_envelope(&response.data).expect("response envelope");
    assert_eq!(echoed, rmpv::Value::Binary(body), "d == limit is admitted and answered");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the admitted request ran once");
}

/// `RNS/Link.py:839-846`: a `Resource { data, metadata: None }` response is still the
/// file-response form. Its data is the raw bytes, never a `[request_id, body]` envelope, and
/// it rides without metadata; `q` and `p` still name the request.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_resource_response_without_metadata_is_raw() {
    let mut pair = loopback_request_pair("request-resource-form-raw-no-metadata").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = random_payload(64);
    let expected = data.clone();
    register_handler(&pair, "/raw", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Resource { data: data.clone(), metadata: None }))
    })
    .await;

    let body = over_mdu_body(&pair).await;
    let (id, _) = send_request_as_resource(&pair, "/raw", rmpv::Value::Binary(body)).await;
    let complete = await_response_resource(&mut connector_events, link_id, id).await;
    assert_eq!(complete.data, expected, "the raw data, not an envelope");
    assert!(complete.metadata.is_none(), "no metadata rides with the resource");
    assert!(
        unpack_response_envelope(&complete.data).is_err(),
        "64 random bytes do not decode as a response envelope"
    );

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "a Resource response never goes out as a packet");
    let [advertisement] = response_advertisements_for(&summary, id)[..] else {
        panic!("exactly one response advertisement names the request");
    };
    assert!(advertisement.is_response() && !advertisement.is_request(), "p set, u clear");
    assert_eq!(advertisement.data_size, expected.len() as u64, "d is the raw data length");
}

/// `auto_compress` is per registration (`RNS/Link.py:846`, `:852`): the same compressible
/// response leaves compressed from a path registered with it on and uncompressed from a
/// path registered with it off, and both still travel as response Resources.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_response_resource_honours_auto_compress() {
    let mut pair = loopback_request_pair("request-resource-form-auto-compress").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let zeros = over_mdu_zeros(&pair).await;
    let expected = zeros.clone();
    for (path, auto_compress) in [("/zip", true), ("/flat", false)] {
        let zeros = zeros.clone();
        register_handler_with_compression(&pair, path, auto_compress, move |_: InboundRequest| {
            std::future::ready(Some(RequestResponse::Bytes(zeros.clone())))
        })
        .await;
    }

    let body = over_mdu_body(&pair).await;
    let mut ids = Vec::new();
    for path in ["/zip", "/flat"] {
        let (id, _) =
            send_request_as_resource(&pair, path, rmpv::Value::Binary(body.clone())).await;
        let complete = await_response_resource(&mut connector_events, link_id, id).await;
        let (answered, value) =
            unpack_response_envelope(&complete.data).expect("response envelope");
        assert_eq!(answered, id.to_bytes());
        assert_eq!(value, rmpv::Value::Binary(expected.clone()), "{path} echoes the zeros intact");
        ids.push(id);
    }

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "both responses outgrow the MDU");
    let [zip] = response_advertisements_for(&summary, ids[0])[..] else {
        panic!("exactly one response advertisement for /zip");
    };
    let [flat] = response_advertisements_for(&summary, ids[1])[..] else {
        panic!("exactly one response advertisement for /flat");
    };
    assert!(zip.compressed(), "auto_compress on: c is set");
    assert!(zip.transfer_size < zip.data_size, "auto_compress on: t is under d");
    assert!(!flat.compressed(), "auto_compress off: c is clear");
    // `t` is the encrypted transfer size, so it sits a little over `d` when nothing was
    // compressed; the compressed transfer is the one that is far smaller.
    assert!(flat.transfer_size >= flat.data_size, "auto_compress off: t is not under d");
    assert!(zip.transfer_size < flat.transfer_size, "the compressed transfer is smaller");
    assert_eq!(zip.data_size, flat.data_size, "the same envelope went out both times");
}

/// A request Resource for a path with no handler is assembled, posts `Complete` for
/// broadcast consumers, and is answered by nothing; the registered path on the same link
/// still answers afterwards.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_unregistered_path_is_silent() {
    let mut pair = loopback_request_pair("request-resource-form-unregistered").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone())).await;

    let body = over_mdu_body(&pair).await;
    let (silent, hash) =
        send_request_as_resource(&pair, "/nowhere", rmpv::Value::Binary(body.clone())).await;
    let complete = await_request_resource_complete(&mut listener_events, link_id).await;
    assert_eq!(complete.request_id.as_deref(), Some(silent.as_bytes().as_slice()));
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(matches!(outcome, ResourceEventKind::OutboundComplete), "accepted, got {outcome:?}");

    let (answered, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body)).await;
    await_response_resource(&mut connector_events, link_id, answered).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "only the registered path ran a handler");

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0, "no response packet for either request");
    assert!(response_advertisements_for(&summary, silent).is_empty(), "nothing answered /nowhere");
    assert_eq!(response_advertisements_for(&summary, answered).len(), 1, "/echo was answered once");
}

/// `RNS/Link.py:818-824` on the resource path: a listed peer is denied while the link holds
/// no identity and answered on the same link once it identifies.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_allow_list_denies_until_identified() {
    let mut pair = loopback_request_pair("request-resource-form-allow-list").await;
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let listed = identity.as_identity().address_hash;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(
        &pair,
        "/echo",
        RequestAllow::List(HashSet::from([listed])),
        counting_echo_handler(calls.clone()),
    )
    .await;

    let body = over_mdu_body(&pair).await;
    let (denied, hash) =
        send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body.clone())).await;
    let outcome = await_outbound_terminal(&mut connector_events, hash).await;
    assert!(matches!(outcome, ResourceEventKind::OutboundComplete), "accepted, got {outcome:?}");

    assert_eq!(identify_connector_as(&mut pair, &identity).await, listed);
    let (answered, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body)).await;
    await_response_resource(&mut connector_events, link_id, answered).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the handler ran only for the identified peer");

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0);
    assert!(response_advertisements_for(&summary, denied).is_empty(), "unidentified: denied");
    assert_eq!(response_advertisements_for(&summary, answered).len(), 1, "identified: answered");
}

/// A request Resource whose assembled bytes are not a request frame (`RNS/Link.py:888`
/// raises on them) is dropped after `Complete` posts: no handler runs, nothing is answered,
/// and the next well-formed request Resource on the same link is answered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_undecodable_body_is_dropped() {
    let mut pair = loopback_request_pair("request-resource-form-malformed").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone())).await;

    let mut dropped = Vec::new();
    for (label, packed) in undecodable_request_bodies() {
        let id = RequestId::of_packed_request(&packed);
        let hash = send_packed_request_as_resource(&pair, id, packed).await;
        let complete = await_request_resource_complete(&mut listener_events, link_id).await;
        assert_eq!(
            complete.request_id.as_deref(),
            Some(id.as_bytes().as_slice()),
            "Complete posts for the {label} body"
        );
        let outcome = await_outbound_terminal(&mut connector_events, hash).await;
        assert!(
            matches!(outcome, ResourceEventKind::OutboundComplete),
            "the {label} body is accepted and assembled, got {outcome:?}"
        );
        dropped.push(id);
    }

    let body = over_mdu_body(&pair).await;
    let (answered, _) = send_request_as_resource(&pair, "/echo", rmpv::Value::Binary(body)).await;
    await_response_resource(&mut connector_events, link_id, answered).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "only the well-formed request ran a handler");

    let summary = drain_connector_tap(&mut pair).await;
    assert_eq!(summary.response_packets, 0);
    for id in dropped {
        assert!(response_advertisements_for(&summary, id).is_empty(), "no answer to {id}");
    }
    assert_eq!(response_advertisements_for(&summary, answered).len(), 1);

    timeout(STOP_TIMEOUT, pair.handles.listener.transport.stop())
        .await
        .expect("listener stops after undecodable request resources");
    timeout(STOP_TIMEOUT, pair.handles.connector.transport.stop()).await.expect("connector stops");
}
