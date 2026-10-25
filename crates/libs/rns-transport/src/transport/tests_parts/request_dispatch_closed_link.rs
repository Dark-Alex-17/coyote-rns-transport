// Loopback coverage for a response whose link closed while the handler ran. The response
// layer looks the link up after the handler returns and drops the response when the link
// closed in between. Before that lookup existed the packet build only warned about the
// closed link and carried on: `finalize_local_close` had reset the session key to
// `DerivedKey::new_empty()`, so the response was encrypted under the all-zero key and
// sent anyway. Helpers come from the sibling `tests_parts` files.

/// A handler that closes the responder's own `link` before answering with `response`, so
/// the answer reaches the response layer after the close.
fn close_then_answer(
    link: Arc<Mutex<Link>>,
    response: impl Fn() -> RequestResponse + Send + Sync + 'static,
) -> impl RequestHandler + 'static {
    move |_: InboundRequest| {
        let link = link.clone();
        let response = response();
        async move {
            link.lock().await.close();
            Some(response)
        }
    }
}

/// Sends one request to `path` and waits until the responder has either dropped the
/// response for the closed link or warned while building its packet; the warning count
/// must not have moved.
#[deny(clippy::await_holding_invalid_type)]
async fn assert_response_dropped_without_packet_warning(pair: &LoopbackRequestPair, path: &str) {
    let link_id = pair.handles.link_id;
    let before = closed_link_packet_warning_count();
    send_request(pair, path, rmpv::Value::Nil).await;
    wait_until(
        "the response to be dropped",
        || {
            closed_link_packet_warning_count() > before
                || !closed_link_response_records_for(link_id).is_empty()
        },
        || {
            format!(
                "closed-link warnings before={before} now={}",
                closed_link_packet_warning_count()
            )
        },
    )
    .await;
    assert_eq!(
        closed_link_packet_warning_count(),
        before,
        "a response for a link closed before transmit builds no packet"
    );
    let dropped = closed_link_response_records_for(link_id);
    assert_eq!(dropped.len(), 1, "the drop is recorded once: {dropped:?}");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_packet_response_on_a_link_closed_by_the_handler_is_dropped()
{
    install_test_logger();
    let pair = loopback_request_pair("request-closed-link-packet").await;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("responder tracks the inbound link");
    let handler =
        close_then_answer(in_link, || RequestResponse::Value(rmpv::Value::from("too late")));
    register_handler(&pair, "/close-then-answer", RequestAllow::All, handler).await;

    assert_response_dropped_without_packet_warning(&pair, "/close-then-answer").await;
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_response_on_a_link_closed_by_the_handler_is_dropped(
) {
    install_test_logger();
    let pair = loopback_request_pair("request-closed-link-resource").await;
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("responder tracks the inbound link");
    let handler = close_then_answer(in_link, || RequestResponse::Resource {
        data: vec![0x5a; 64],
        metadata: None,
    });
    register_handler(&pair, "/close-then-send", RequestAllow::All, handler).await;

    assert_response_dropped_without_packet_warning(&pair, "/close-then-send").await;
}
