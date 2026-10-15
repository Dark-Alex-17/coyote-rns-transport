// Loopback coverage for how the responder routes a request to a handler: the allow list
// consults the peer identity the link holds at the moment each request arrives
// (`RNS/Link.py:818-824`), so a peer the list names is denied until it identifies and
// answered afterwards on the same link; a second `register_request_handler` for a path
// replaces the whole row, allow mode included (`RNS/Destination.py:398`); and the handler
// table belongs to one destination (`RNS/Destination.py:398`, `RNS/Link.py:810`), so a link
// to a sibling destination on the same transport stays silent for the same path until that
// destination registers its own handler. Helpers come from the sibling `tests_parts` files.

/// Spacing between two announces the listener sends to the connector's fresh interface, so
/// the second one is not held by announce ingress control.
const SIBLING_ANNOUNCE_SPACING: Duration = Duration::from_secs(2);

/// Identifies the connector to the responder as `identity` and returns its hash once the
/// responder has posted `PeerIdentified` for the pair's link.
#[deny(clippy::await_holding_invalid_type)]
async fn identify_connector_as(
    pair: &mut LoopbackRequestPair,
    identity: &PrivateIdentity,
) -> AddressHash {
    let packet = {
        let link = pair.handles.link.lock().await;
        link.identify_packet(&link.identify_payload(identity)).expect("identify fits a packet")
    };
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "identify reaches the wire");
    let link_id = pair.handles.link_id;
    await_event(&mut pair.handles.in_events, |event| {
        event.id == link_id && matches!(event.event, LinkEvent::PeerIdentified(_))
    })
    .await;
    identity.as_identity().address_hash
}

/// Like `send_request`, for any of the connector's links.
#[deny(clippy::await_holding_invalid_type)]
async fn send_request_on_link(
    pair: &LoopbackRequestPair,
    link: &Arc<Mutex<Link>>,
    path: &str,
    value: rmpv::Value,
) -> RequestId {
    let packed = Link::request_payload(path, value).expect("request packs").packed;
    let packet = link.lock().await.request_packet(&packed).expect("request fits a packet");
    let id = RequestId::from_packet_hash(&packet.hash());
    let outcome =
        pair.handles.connector.transport.send_link_packet_on_bound_iface(link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "request reaches the wire");
    id
}

/// Like `await_response`, for any of the connector's links.
#[deny(clippy::await_holding_invalid_type)]
async fn await_response_on_link(
    pair: &mut LoopbackRequestPair,
    link_id: AddressHash,
    id: RequestId,
) -> rmpv::Value {
    let event = timeout(
        RESPONSE_TIMEOUT,
        await_event(&mut pair.out_events, |event| {
            event.id == link_id
                && matches!(&event.event, LinkEvent::Data(payload)
                    if payload.context() == PacketContext::Response)
        }),
    )
    .await
    .expect("response arrives");
    let LinkEvent::Data(payload) = event.event else { unreachable!("matched above") };
    let (answered, body) = unpack_response_envelope(payload.as_slice()).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    body
}

/// Like `await_request_data_event`, for any of the listener's in-links.
#[deny(clippy::await_holding_invalid_type)]
async fn await_request_data_event_on_link(
    pair: &mut LoopbackRequestPair,
    link_id: AddressHash,
    id: RequestId,
) {
    await_event(&mut pair.handles.in_events, |event| {
        event.id == link_id
            && matches!(&event.event, LinkEvent::Data(payload)
                if payload.context() == PacketContext::Request
                    && payload.request_id() == Some(id.to_bytes()))
    })
    .await;
}

/// `if self.__remote_identity.hash in allowed_list` reads the link's identity when each
/// request arrives (`RNS/Link.py:821`): the same listed peer is denied while it is still
/// anonymous and answered once it has identified, without re-registering anything.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_list_checks_the_identity_per_request() {
    let mut pair = loopback_request_pair("request-allow-list-late-identify").await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let listed = identity.as_identity().address_hash;
    register_handler(&pair, "/echo", RequestAllow::List(HashSet::from([listed])), echo_handler())
        .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("anonymous")).await;
    await_request_data_event(&mut pair, id).await;
    assert_no_response(&mut pair).await;

    assert_eq!(identify_connector_as(&mut pair, &identity).await, listed);
    let id = send_request(&pair, "/echo", rmpv::Value::from("identified")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("identified"));
}

/// `self.request_handlers[path_hash] = request_handler` (`RNS/Destination.py:398`) stores
/// the handler together with its allow mode, so re-registering a path with `ALLOW_NONE`
/// silences it even though the previous row allowed everyone.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_reregistration_replaces_the_allow_mode() {
    let mut pair = loopback_request_pair("request-reregister-allow").await;
    let calls = Arc::new(AtomicUsize::new(0));
    register_handler(&pair, "/echo", RequestAllow::All, counting_echo_handler(calls.clone()))
        .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("open")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("open"));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the open row ran once");
    let link_id = pair.handles.link_id;
    assert_eq!(
        count_link_packets(&mut pair.connector_tap, link_id, PacketContext::Response),
        1,
        "exactly one response packet answered the open request"
    );

    register_handler(&pair, "/echo", RequestAllow::None, counting_echo_handler(calls.clone()))
        .await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("closed")).await;
    await_request_data_event(&mut pair, id).await;
    assert_no_response(&mut pair).await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the replaced row denies before any handler");
}

/// Handlers hang off the destination that owns the link (`RNS/Link.py:810`): a path
/// registered on one destination is not consulted for a link to a sibling destination on
/// the same transport, which still posts `Data(Request)` and answers only once it has
/// registered the path itself.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_is_scoped_to_its_destination() {
    let mut pair = loopback_request_pair("request-scope-a").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    // The connector's interface is new, so a second announce arriving within a fraction of
    // a second of the first would trip ingress control and be held for minutes
    // (`RNS/Interfaces/Interface.py:76-77`, `:188-205`). Two samples `gap` apart measure
    // as `2 / gap` Hz; two seconds keeps the sibling's announce well under the new-interface
    // burst threshold.
    tokio::time::sleep(SIBLING_ANNOUNCE_SPACING).await;
    let (sibling, sibling_desc) =
        register_in_destination(&pair.handles.listener.transport, "loopback", "request-scope-b")
            .await;
    let learned =
        learn_announce(&mut pair.handles.connector.announces, &sibling_desc.address_hash).await;
    let (sibling_link, sibling_link_id) = open_link(&pair.handles.connector.transport, learned).await;
    assert_ne!(sibling_link_id, pair.handles.link_id, "two distinct links");

    // The sibling link carries the request to the transport and posts it, but no handler
    // on the sibling destination answers it.
    let unanswered = send_request_on_link(&pair, &sibling_link, "/echo", rmpv::Value::from(1)).await;
    await_request_data_event_on_link(&mut pair, sibling_link_id, unanswered).await;

    // The first destination's handler still answers on its own link; the single dispatch
    // worker handles the sibling's request before this one, so this answer bounds it.
    let id = send_request(&pair, "/echo", rmpv::Value::from(2)).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from(2));
    assert_eq!(
        count_link_packets(&mut pair.connector_tap, sibling_link_id, PacketContext::Response),
        0,
        "no response packet on the sibling link"
    );

    // Once the sibling destination registers the path, the same link is answered.
    sibling
        .lock()
        .await
        .register_request_handler("/echo", Arc::new(echo_handler()), RequestAllow::All, true)
        .expect("handler registers");
    let id = send_request_on_link(&pair, &sibling_link, "/echo", rmpv::Value::from(3)).await;
    assert_eq!(
        await_response_on_link(&mut pair, sibling_link_id, id).await,
        rmpv::Value::from(3)
    );
}
