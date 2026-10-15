// Loopback coverage for the consumer-facing promises of the request handler layer that the
// allow-mode and isolation fixtures leave open: a path without a handler stays silent, a
// second registration answers in place of the first, a deregistered path falls silent, the
// handler sees the request fields `RNS/Link.py:831` hands a response generator, handlers
// for one link run concurrently, a hand-written `RequestHandler` impl answers like a
// closure, the file-response form (`RNS/Link.py:839-846`) always travels as a resource with
// its metadata, and a response that exactly fills `Link::link_mdu()` still goes out as one
// packet (`RNS/Link.py:849`). Helpers come from the sibling `tests_parts` files.

use crate::destination::request::{PathHash, ResponseFrame};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

/// How many requests the concurrency test releases through one barrier. Small enough to
/// stay well inside the link event broadcast capacity, large enough that a serialising
/// dispatcher would deadlock on the barrier.
const CONCURRENT_REQUESTS: usize = 6;

/// Collects the response bodies for every id in `ids` from the connector's link events,
/// in whatever order they arrive.
#[deny(clippy::await_holding_invalid_type)]
async fn collect_responses(
    pair: &mut LoopbackRequestPair,
    ids: &[RequestId],
) -> HashMap<RequestId, rmpv::Value> {
    let link_id = pair.handles.link_id;
    let mut bodies = HashMap::new();
    timeout(RESPONSE_TIMEOUT, async {
        while bodies.len() < ids.len() {
            match pair.out_events.recv().await {
                Ok(event) if event.id == link_id => {
                    if let LinkEvent::Data(payload) = &event.event {
                        if payload.context() == PacketContext::Response {
                            let (answered, body) = unpack_response_envelope(payload.as_slice())
                                .expect("response envelope");
                            bodies.insert(RequestId::new(answered), body);
                        }
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => panic!("link event channel closed"),
            }
        }
    })
    .await
    .expect("every request is answered");
    bodies
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_unregistered_path_is_silent_and_registered_path_still_answers(
) {
    let mut pair = loopback_request_pair("request-unregistered-path").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    send_request(&pair, "/not-registered", rmpv::Value::from("lost")).await;
    assert_no_response(&mut pair).await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("still here")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("still here"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_reregistration_answers_with_the_new_handler() {
    let mut pair = loopback_request_pair("request-reregister").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("first")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("first"));

    register_handler(&pair, "/echo", RequestAllow::All, |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Value(rmpv::Value::from("replaced"))))
    })
    .await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("second")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("replaced"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_deregistered_path_falls_silent() {
    let mut pair = loopback_request_pair("request-deregister").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let id = send_request(&pair, "/echo", rmpv::Value::from("before")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("before"));
    // The answered request left exactly one response packet on the connector's tap;
    // account for it so the silence check below starts from a clean tap.
    assert_eq!(
        count_link_packets(&mut pair.connector_tap, pair.handles.link_id, PacketContext::Response),
        1,
        "the answered request produced one response packet"
    );

    assert!(
        pair.handles.destination.lock().await.deregister_request_handler("/echo"),
        "a registered path deregisters"
    );
    assert!(
        !pair.handles.destination.lock().await.deregister_request_handler("/echo"),
        "a second deregistration finds nothing"
    );

    send_request(&pair, "/echo", rmpv::Value::from("after")).await;
    assert_no_response(&mut pair).await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_sees_the_request_fields_of_an_unidentified_peer(
) {
    let mut pair = loopback_request_pair("request-fields").await;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel::<InboundRequest>();
    register_handler(&pair, "/inspect", RequestAllow::All, move |request: InboundRequest| {
        let seen_tx = seen_tx.clone();
        async move {
            seen_tx.send(request).expect("test receiver is alive");
            Some(RequestResponse::Value(rmpv::Value::Nil))
        }
    })
    .await;

    let before = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs_f64();
    let data = rmpv::Value::Map(vec![(rmpv::Value::from("k"), rmpv::Value::from(42))]);
    let id = send_request(&pair, "/inspect", data.clone()).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::Nil);
    let after = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs_f64();

    let seen = seen_rx.recv().await.expect("handler ran once");
    assert_eq!(seen.path(), "/inspect");
    assert_eq!(seen.path_hash(), PathHash::of("/inspect"));
    assert_eq!(seen.data(), &data);
    assert_eq!(seen.request_id(), id);
    assert_eq!(seen.link_id(), pair.handles.link_id);
    assert!(seen.remote_identity().is_none(), "nobody identified on this link");
    // The requester stamps `requested_at` from its clock (`RNS/Link.py:486`); both ends
    // share this host, so a generous window around the exchange bounds it.
    assert!(
        seen.requested_at() >= before - 60.0 && seen.requested_at() <= after + 60.0,
        "requested_at {} is near the exchange [{before}, {after}]",
        seen.requested_at()
    );
    assert_eq!(seen.into_data(), data);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_sees_the_identified_peer() {
    let mut pair = loopback_request_pair("request-fields-identified").await;
    let peer = identify_connector(&mut pair).await;
    let (seen_tx, mut seen_rx) = tokio::sync::mpsc::unbounded_channel::<Option<AddressHash>>();
    register_handler(&pair, "/whoami", RequestAllow::All, move |request: InboundRequest| {
        let seen_tx = seen_tx.clone();
        async move {
            seen_tx
                .send(request.remote_identity().map(|identity| identity.address_hash))
                .expect("test receiver is alive");
            Some(RequestResponse::Value(rmpv::Value::Nil))
        }
    })
    .await;

    let id = send_request(&pair, "/whoami", rmpv::Value::Nil).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::Nil);
    assert_eq!(seen_rx.recv().await.expect("handler ran once"), Some(peer));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handlers_on_one_link_run_concurrently() {
    let mut pair = loopback_request_pair("request-concurrent").await;
    // Every invocation must reach the barrier before any of them answers: a dispatcher
    // that ran handlers one after another would never release it.
    let barrier = Arc::new(tokio::sync::Barrier::new(CONCURRENT_REQUESTS));
    register_handler(&pair, "/gate", RequestAllow::All, move |request: InboundRequest| {
        let barrier = barrier.clone();
        async move {
            barrier.wait().await;
            Some(RequestResponse::Value(request.into_data()))
        }
    })
    .await;

    let mut ids = Vec::with_capacity(CONCURRENT_REQUESTS);
    for n in 0..CONCURRENT_REQUESTS {
        ids.push(send_request(&pair, "/gate", rmpv::Value::from(n as u64)).await);
    }
    let bodies = collect_responses(&mut pair, &ids).await;
    for (n, id) in ids.iter().enumerate() {
        assert_eq!(bodies.get(id), Some(&rmpv::Value::from(n as u64)), "request {n} is answered");
    }
}

/// A hand-written handler: counts its invocations and answers with the count.
struct CountingHandler {
    calls: AtomicUsize,
}

impl RequestHandler for CountingHandler {
    fn handle(
        &self,
        _request: InboundRequest,
    ) -> Pin<Box<dyn Future<Output = Option<RequestResponse>> + Send + '_>> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Box::pin(async move { Some(RequestResponse::Value(rmpv::Value::from(call as u64))) })
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_trait_impl_handler_answers_like_a_closure() {
    let mut pair = loopback_request_pair("request-trait-impl").await;
    register_handler(
        &pair,
        "/count",
        RequestAllow::All,
        CountingHandler { calls: AtomicUsize::new(0) },
    )
    .await;

    let id = send_request(&pair, "/count", rmpv::Value::Nil).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from(1u64));
    let id = send_request(&pair, "/count", rmpv::Value::Nil).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from(2u64));
}

/// The file-response form travels as a resource whatever its size, carrying the raw data
/// and the metadata beside it (`RNS/Link.py:839-846`), not the `[request_id, body]`
/// envelope of a plain response.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_carries_raw_data_and_metadata() {
    let pair = loopback_request_pair("request-resource-form").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    let data = b"small enough for one packet".to_vec();
    let expected = data.clone();
    let metadata = rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("a.txt"))]);
    let expected_metadata = metadata.clone();
    register_handler(&pair, "/file", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Resource {
            data: data.clone(),
            metadata: Some(metadata.clone()),
        }))
    })
    .await;

    let id = send_request(&pair, "/file", rmpv::Value::Nil).await;
    let complete = await_resource_event(&mut resource_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(id.as_bytes().as_slice()));
    assert_eq!(complete.data, expected, "the raw data, not an envelope");
    let packed = complete.metadata.expect("metadata rides with the resource");
    let decoded = rmpv::decode::read_value(&mut packed.as_slice()).expect("metadata is msgpack");
    assert_eq!(decoded, expected_metadata);
}

/// `len(packed_response) <= self.mdu` (`RNS/Link.py:849`): the largest envelope that fits
/// still goes out as one packet, and only the next byte tips it into a resource.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_response_that_exactly_fills_the_mdu_is_one_packet() {
    let mut pair = loopback_request_pair("request-mdu-boundary").await;
    let mdu = pair.handles.link.lock().await.link_mdu();
    let envelope_len = |body_len: usize| {
        ResponseFrame::new(RequestId::new([0u8; 16]), rmpv::Value::Binary(vec![0u8; body_len]))
            .encode()
            .len()
    };
    let fit = (0..=mdu).rev().find(|&len| envelope_len(len) <= mdu).expect("some body fits");
    assert!(envelope_len(fit + 1) > mdu, "fit is the boundary");
    let body = random_payload(fit);
    let expected = body.clone();
    register_handler(&pair, "/edge", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(body.clone())))
    })
    .await;

    let id = send_request(&pair, "/edge", rmpv::Value::Nil).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::Binary(expected));
}
