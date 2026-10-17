// Usage-pattern coverage for `max_response_size` as a consumer sizes it: the limit is an
// inclusive bound on what the receipt later reports as `response_size()`, in the packet
// form as in the Resource form; several Resource responses in flight on one link each
// answer to their own limit (`RNS/Link.py:1046-1053` walks the pending requests by `q`),
// so one refused response neither disturbs its neighbours nor leaves a limit behind for
// the next request on the link. Helpers come from the sibling `tests_parts` files.

/// A body `extra` bytes over the link MDU, so the request and its echo both travel as
/// Resources and distinct `extra`s give distinct advertised `d`s.
#[deny(clippy::await_holding_invalid_type)]
async fn over_mdu_body_plus(pair: &LoopbackRequestPair, extra: usize) -> Vec<u8> {
    random_payload(pair.handles.link.lock().await.link_mdu() + 1 + extra)
}

/// An `/echo` of `body` under `limit`.
#[deny(clippy::await_holding_invalid_type)]
async fn echo_under_limit(pair: &LoopbackRequestPair, body: &[u8], limit: usize) -> RequestReceipt {
    request_with(
        pair,
        "/echo",
        rmpv::Value::Binary(body.to_vec()),
        RequestOptions::default().with_max_response_size(limit),
    )
    .await
}

/// Packet form: a limit equal to the `response_size()` an unlimited request reports is
/// admitted, one byte under it fails `Oversize`, and the link answers in between.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe3_packet_limit_is_inclusive_of_the_reported_size(
) {
    let pair = loopback_request_pair("sizes-probe3-packet-boundary").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let value = rmpv::Value::from("boundary");

    let unlimited = request(&pair, "/echo", value.clone()).await;
    let answer = await_receipt(&unlimited).await.expect("unlimited echo answers");
    assert_eq!(answer.value(), Some(&value));
    let reported = unlimited.response_size().expect("a ready receipt reports its size");
    assert!(reported > 1, "a measurable packet response: {reported}");

    let at_limit = request_with(
        &pair,
        "/echo",
        value.clone(),
        RequestOptions::default().with_max_response_size(reported),
    )
    .await;
    let answer = await_receipt(&at_limit).await.expect("response_size == limit is admitted");
    assert_eq!(answer.value(), Some(&value));
    assert_eq!(at_limit.status(), RequestStatus::Ready);
    assert_eq!(at_limit.response_size(), Some(reported));

    let under_limit = request_with(
        &pair,
        "/echo",
        value.clone(),
        RequestOptions::default().with_max_response_size(reported - 1),
    )
    .await;
    assert_eq!(await_receipt(&under_limit).await, Err(RequestError::Oversize));
    assert_eq!(under_limit.status(), RequestStatus::Failed);

    let after = request(&pair, "/echo", value.clone()).await;
    assert_eq!(await_receipt(&after).await.expect("the link still answers").value(), Some(&value));
}

/// Three Resource responses in flight on one link, each requested under a limit equal to
/// its own envelope length, all resolve `Ready` with their own body and sizes.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe3_concurrent_resource_responses_each_keep_their_own_limit(
) {
    let pair = loopback_request_pair("sizes-probe3-concurrent-limits").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let bodies = [
        over_mdu_body_plus(&pair, 0).await,
        over_mdu_body_plus(&pair, 1_024).await,
        over_mdu_body_plus(&pair, 2_048).await,
    ];
    let limits: Vec<usize> = bodies.iter().map(|body| sizes_envelope_len(body)).collect();
    assert!(limits.windows(2).all(|pair| pair[0] < pair[1]), "distinct d per response: {limits:?}");

    let mut receipts = Vec::with_capacity(bodies.len());
    for (body, limit) in bodies.iter().zip(&limits) {
        receipts.push(echo_under_limit(&pair, body, *limit).await);
    }
    let ids: std::collections::HashSet<RequestId> =
        receipts.iter().map(RequestReceipt::request_id).collect();
    assert_eq!(ids.len(), bodies.len(), "three distinct requests in flight");

    // Await them out of issue order: the responses complete in whatever order the link
    // delivers them.
    for index in [2usize, 0, 1] {
        let receipt = &receipts[index];
        let response = sizes_await_resource_receipt(receipt)
            .await
            .unwrap_or_else(|err| panic!("response {index} under its own limit: {err:?}"));
        assert_eq!(receipt.status(), RequestStatus::Ready);
        assert_eq!(response.into_value(), Some(rmpv::Value::Binary(bodies[index].clone())));
        assert_eq!(receipt.response_size(), Some(limits[index]), "response {index} reports d");
        assert!(
            receipt.response_transfer_size().is_some_and(|t| t > 0),
            "response {index} reports a transfer size"
        );
        assert_eq!(receipt.progress(), 1.0);
    }
}

/// Of three Resource responses in flight, the one requested one byte under its envelope
/// length fails `Oversize` and the responder sees `OutboundRejected` for it, while its two
/// neighbours resolve `Ready`; the failed limit is not left behind: the same body requested
/// again under a covering limit on the same link is admitted.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe3_one_refused_response_leaves_its_neighbours_and_the_link_intact(
) {
    let pair = loopback_request_pair("sizes-probe3-one-refused").await;
    let mut listener_events = pair.handles.listener.transport.resource_events();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let alpha = over_mdu_body_plus(&pair, 0).await;
    let beta = over_mdu_body_plus(&pair, 1_024).await;
    let gamma = over_mdu_body_plus(&pair, 2_048).await;
    let beta_len = sizes_envelope_len(&beta);

    let alpha_receipt = echo_under_limit(&pair, &alpha, sizes_envelope_len(&alpha)).await;
    let beta_receipt = echo_under_limit(&pair, &beta, beta_len - 1).await;
    let gamma_receipt = echo_under_limit(&pair, &gamma, sizes_envelope_len(&gamma)).await;

    assert_eq!(sizes_await_resource_receipt(&beta_receipt).await, Err(RequestError::Oversize));
    assert_eq!(beta_receipt.status(), RequestStatus::Failed);
    let rejected = await_resource_event(&mut listener_events, |event| {
        event.link_id == link_id && matches!(event.kind, ResourceEventKind::OutboundRejected)
    })
    .await;
    assert!(matches!(rejected.kind, ResourceEventKind::OutboundRejected));

    let alpha_body = sizes_await_resource_receipt(&alpha_receipt).await.expect("alpha unaffected");
    assert_eq!(alpha_body.into_value(), Some(rmpv::Value::Binary(alpha)));
    let gamma_body = sizes_await_resource_receipt(&gamma_receipt).await.expect("gamma unaffected");
    assert_eq!(gamma_body.into_value(), Some(rmpv::Value::Binary(gamma)));
    assert_eq!(alpha_receipt.status(), RequestStatus::Ready);
    assert_eq!(gamma_receipt.status(), RequestStatus::Ready);

    let retry = echo_under_limit(&pair, &beta, beta_len).await;
    assert_ne!(retry.request_id(), beta_receipt.request_id(), "a fresh request, a fresh id");
    let retry_body =
        sizes_await_resource_receipt(&retry).await.expect("the covering limit admits the retry");
    assert_eq!(retry_body.into_value(), Some(rmpv::Value::Binary(beta)));
    assert_eq!(retry.response_size(), Some(beta_len));
}
