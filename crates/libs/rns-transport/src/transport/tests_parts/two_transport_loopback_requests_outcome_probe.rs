// Usage probes for `InboundRequest::track_outcome` beyond the verdict table in
// `two_transport_loopback_requests_outcome.rs`: a handler that drops its receiver without
// awaiting it still gets its response delivered (packet and Resource alike); a link the
// handler itself closed before answering resolves `NotSent` for both response forms; a
// handler that awaits its own receiver inside `handle` wedges only that request, the link
// keeps answering others; and a `track_outcome` call made after the handler has returned
// resolves `NotSent` once the request drops, staying pending while it lives. Helpers come
// from the sibling `tests_parts` files; `close_then_answer` is not reused because it never
// sees the request and so cannot call `track_outcome`.

/// A handler that tracks the outcome and drops the receiver on the spot, the way a
/// consumer that only wanted the side effects of `track_outcome` would.
fn tracking_then_dropping_handler(
    respond: impl Fn(InboundRequest) -> Option<RequestResponse> + Send + Sync,
) -> impl Fn(InboundRequest) -> std::future::Ready<Option<RequestResponse>> + Send + Sync {
    move |mut request: InboundRequest| {
        drop(request.track_outcome());
        std::future::ready(respond(request))
    }
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_dropped_outcome_receiver_still_delivers_the_packet_response(
) {
    let mut pair = loopback_request_pair("request-outcome-probe-dropped-packet").await;
    register_handler(
        &pair,
        "/echo",
        RequestAllow::All,
        tracking_then_dropping_handler(|request| Some(RequestResponse::Value(request.into_data()))),
    )
    .await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("untracked after all")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("untracked after all"));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_dropped_outcome_receiver_still_delivers_the_resource_response(
) {
    let pair = loopback_request_pair("request-outcome-probe-dropped-resource").await;
    let mut resource_events = pair.handles.connector.transport.resource_events();
    let link_id = pair.handles.link_id;
    register_handler(
        &pair,
        "/file",
        RequestAllow::All,
        tracking_then_dropping_handler(|_| {
            Some(RequestResponse::Resource { data: b"still delivered".to_vec(), metadata: None })
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
}

/// The handler closes the responder's own in-link before answering with a packet-size
/// `Value`: the response layer finds the link no longer active and resolves `NotSent`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_on_a_link_closed_by_the_handler_is_not_sent(
) {
    let pair = loopback_request_pair("request-outcome-probe-closed-packet").await;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("responder tracks the inbound link");
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/close-then-answer",
        RequestAllow::All,
        move |mut request: InboundRequest| {
            outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
            let link = in_link.clone();
            async move {
                link.lock().await.close();
                Some(RequestResponse::Value(rmpv::Value::from("too late")))
            }
        },
    )
    .await;

    send_request(&pair, "/close-then-answer", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);
}

/// Same close, `Resource` form: the transfer never registers, so the verdict is `NotSent`
/// rather than a `Resource(Failed(..))`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_on_a_link_closed_by_the_handler_is_not_sent(
) {
    let pair = loopback_request_pair("request-outcome-probe-closed-resource").await;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("responder tracks the inbound link");
    let (outcome_tx, mut outcomes) = tokio::sync::mpsc::unbounded_channel();
    register_handler(
        &pair,
        "/close-then-send",
        RequestAllow::All,
        move |mut request: InboundRequest| {
            outcome_tx.send(request.track_outcome()).expect("test receiver is alive");
            let link = in_link.clone();
            async move {
                link.lock().await.close();
                Some(RequestResponse::Resource { data: vec![0x5a; 64], metadata: None })
            }
        },
    )
    .await;

    send_request(&pair, "/close-then-send", rmpv::Value::Nil).await;
    assert_eq!(await_outcome(&mut outcomes).await, ResponseOutcome::NotSent);
}

/// A handler that awaits its own receiver before returning can never be resolved (the
/// dispatcher resolves after the handler returns), but each request runs in its own task,
/// so only that request is wedged: a second request on the same link is answered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_handler_awaiting_its_own_outcome_wedges_only_itself() {
    let mut pair = loopback_request_pair("request-outcome-probe-self-await").await;
    let (entered_tx, entered) = tokio::sync::oneshot::channel::<()>();
    let entered_tx = std::sync::Mutex::new(Some(entered_tx));
    register_handler(
        &pair,
        "/self-await",
        RequestAllow::All,
        move |mut request: InboundRequest| {
            let receiver = request.track_outcome();
            if let Some(tx) = entered_tx.lock().expect("entered slot").take() {
                tx.send(()).expect("test is waiting");
            }
            async move {
                let _ = receiver.await;
                Some(RequestResponse::Value(request.into_data()))
            }
        },
    )
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    send_request(&pair, "/self-await", rmpv::Value::from("wedged")).await;
    timeout(RESPONSE_TIMEOUT, entered).await.expect("handler ran").expect("handler ran");
    assert_no_response(&mut pair).await;

    let id = send_request(&pair, "/echo", rmpv::Value::from("still served")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("still served"));
}

/// `track_outcome` after the handler has returned arms a sender the dispatcher has already
/// looked for: the receiver stays pending while the request lives and resolves `NotSent`
/// once the request drops.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_track_outcome_after_the_handler_returned_resolves_not_sent_on_drop(
) {
    let mut pair = loopback_request_pair("request-outcome-probe-late-track").await;
    let stash: Arc<std::sync::Mutex<Option<InboundRequest>>> =
        Arc::new(std::sync::Mutex::new(None));
    let handler_stash = stash.clone();
    register_handler(&pair, "/stash", RequestAllow::All, move |request: InboundRequest| {
        let response = RequestResponse::Value(request.data().clone());
        *handler_stash.lock().expect("stash slot") = Some(request);
        std::future::ready(Some(response))
    })
    .await;

    let id = send_request(&pair, "/stash", rmpv::Value::from("kept")).await;
    assert_eq!(await_response(&mut pair, id).await, rmpv::Value::from("kept"));

    let mut request =
        stash.lock().expect("stash slot").take().expect("handler stashed the request");
    let mut receiver = request.track_outcome();
    assert!(
        timeout(Duration::from_millis(200), &mut receiver).await.is_err(),
        "a late receiver stays pending while the request is alive"
    );
    drop(request);
    assert_eq!(
        timeout(RESPONSE_TIMEOUT, receiver).await.expect("outcome resolves"),
        ResponseOutcome::NotSent
    );
}
