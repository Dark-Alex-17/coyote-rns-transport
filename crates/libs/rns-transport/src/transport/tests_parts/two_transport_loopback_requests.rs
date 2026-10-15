// Loopback coverage for the responder side of the request layer: a handler registered on
// the listener's destination answers, or stays silent, for a request the connector sends
// as one link packet. The allow modes follow `RNS/Link.py:818-824`, `None` from the handler
// sends nothing (`RNS/Link.py:844`), and the `LinkEvent::Data(Request)` broadcast keeps
// posting for a handled request. Helpers come from the sibling `tests_parts` files.

use crate::destination::link::unpack_response_envelope;
use crate::destination::request::{
    InboundRequest, RequestAllow, RequestHandler, RequestId, RequestResponse,
};
use std::collections::HashSet;

/// Bounds the wait for a response packet. Generous against a loaded host, far below the
/// slow-handler sleep in the isolation tests.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// A link pair with the connector's response taps subscribed before any request goes out,
/// so a response that must not exist cannot slip past the silence check.
struct LoopbackRequestPair {
    handles: LoopbackLinkHandles,
    out_events: broadcast::Receiver<LinkEventData>,
    connector_tap: broadcast::Receiver<RxMessage>,
}

#[deny(clippy::await_holding_invalid_type)]
async fn loopback_request_pair(aspect: &str) -> LoopbackRequestPair {
    let handles = loopback_link_handles(aspect).await;
    let out_events = handles.connector.transport.out_link_events();
    let connector_tap = handles.connector.transport.iface_rx();
    LoopbackRequestPair { handles, out_events, connector_tap }
}

#[deny(clippy::await_holding_invalid_type)]
async fn register_handler(
    pair: &LoopbackRequestPair,
    path: &str,
    allow: RequestAllow,
    handler: impl RequestHandler + 'static,
) {
    pair.handles
        .destination
        .lock()
        .await
        .register_request_handler(path, Arc::new(handler), allow, true)
        .expect("handler registers");
}

fn echo_handler(
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    |request: InboundRequest| std::future::ready(Some(RequestResponse::Value(request.into_data())))
}

/// Sends `value` to `path` as a single request packet and returns the id the responder
/// derives from that packet.
#[deny(clippy::await_holding_invalid_type)]
async fn send_request(pair: &LoopbackRequestPair, path: &str, value: rmpv::Value) -> RequestId {
    let packed = Link::request_payload(path, value).expect("request packs").packed;
    let packet =
        pair.handles.link.lock().await.request_packet(&packed).expect("request fits a packet");
    let id = RequestId::from_packet_hash(&packet.hash());
    let outcome = pair
        .handles
        .connector
        .transport
        .send_link_packet_on_bound_iface(&pair.handles.link, packet)
        .await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "request reaches the wire");
    id
}

/// Waits for the response packet answering `id` on the connector's link events and
/// returns its body.
#[deny(clippy::await_holding_invalid_type)]
async fn await_response(pair: &mut LoopbackRequestPair, id: RequestId) -> rmpv::Value {
    let link_id = pair.handles.link_id;
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

/// Bounds the window with a data round trip in both directions, then checks that no
/// response packet reached the connector's interface or its link events.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_no_response(pair: &mut LoopbackRequestPair) {
    exchange_data_both_ways(&mut pair.handles).await;
    let link_id = pair.handles.link_id;
    assert_eq!(
        count_link_packets(&mut pair.connector_tap, link_id, PacketContext::Response),
        0,
        "no response packet on the wire"
    );
    loop {
        match pair.out_events.try_recv() {
            Ok(event) => assert!(
                !(event.id == link_id
                    && matches!(&event.event, LinkEvent::Data(payload)
                        if payload.context() == PacketContext::Response)),
                "no response event on the link"
            ),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }
}

/// Identifies the connector to the responder with a fresh identity and returns its hash
/// once the responder has posted `PeerIdentified`.
#[deny(clippy::await_holding_invalid_type)]
async fn identify_connector(pair: &mut LoopbackRequestPair) -> AddressHash {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let packet = {
        let link = pair.handles.link.lock().await;
        link.identify_packet(&link.identify_payload(&identity)).expect("identify fits a packet")
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_all_is_answered_and_still_posts_data_event() {
    let mut pair = loopback_request_pair("request-allow-all").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("ping")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("ping"));

    let link_id = pair.handles.link_id;
    let observed = await_event(&mut pair.handles.in_events, |event| {
        event.id == link_id
            && matches!(&event.event, LinkEvent::Data(payload)
                if payload.context() == PacketContext::Request)
    })
    .await;
    let LinkEvent::Data(payload) = observed.event else { unreachable!("matched above") };
    assert_eq!(
        payload.request_id(),
        Some(id.to_bytes()),
        "a handled request still posts its Data event"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_none_is_denied_silently() {
    let mut pair = loopback_request_pair("request-allow-none").await;
    register_handler(&pair, "/echo", RequestAllow::None, echo_handler()).await;

    send_request(&pair, "/echo", rmpv::Value::from("ping")).await;
    assert_no_response(&mut pair).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_list_answers_listed_peer() {
    let mut pair = loopback_request_pair("request-allow-list").await;
    let peer = identify_connector(&mut pair).await;
    register_handler(&pair, "/echo", RequestAllow::List(HashSet::from([peer])), echo_handler())
        .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from(7)).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from(7));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_list_denies_unlisted_peer() {
    let mut pair = loopback_request_pair("request-allow-list-unlisted").await;
    identify_connector(&mut pair).await;
    let someone_else = PrivateIdentity::new_from_rand(OsRng).as_identity().address_hash;
    register_handler(
        &pair,
        "/echo",
        RequestAllow::List(HashSet::from([someone_else])),
        echo_handler(),
    )
    .await;

    send_request(&pair, "/echo", rmpv::Value::from(7)).await;
    assert_no_response(&mut pair).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_allow_list_denies_unidentified_peer() {
    let mut pair = loopback_request_pair("request-allow-list-unidentified").await;
    let listed = PrivateIdentity::new_from_rand(OsRng).as_identity().address_hash;
    register_handler(&pair, "/echo", RequestAllow::List(HashSet::from([listed])), echo_handler())
        .await;

    send_request(&pair, "/echo", rmpv::Value::from(7)).await;
    assert_no_response(&mut pair).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_none_sends_nothing() {
    let mut pair = loopback_request_pair("request-handler-none").await;
    register_handler(&pair, "/quiet", RequestAllow::All, |_: InboundRequest| {
        std::future::ready(None::<RequestResponse>)
    })
    .await;

    send_request(&pair, "/quiet", rmpv::Value::Nil).await;
    assert_no_response(&mut pair).await;
}

/// A response that does not fit one link packet travels as a response resource
/// (`RNS/Link.py:851-852`) carrying the same `[request_id, body]` envelope.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_oversize_response_arrives_as_resource() {
    let pair = loopback_request_pair("request-oversize").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let body = random_payload(pair.handles.link.lock().await.link_mdu() + 1);
    let expected = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let id = send_request(&pair, "/big", rmpv::Value::Nil).await;
    let complete = await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(id.as_bytes().as_slice()));
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(expected));
}
