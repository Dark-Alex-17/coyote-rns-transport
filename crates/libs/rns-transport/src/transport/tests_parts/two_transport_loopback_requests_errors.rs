// Loopback coverage for the errors `Transport::request` returns before any request leaves:
// a link the connector knows but that is not `Active` is refused with `ConnectionError`,
// where Python drops the packet only on a `CLOSED` link (`RNS/Packet.py:294-295`). Helpers
// come from the sibling `tests_parts` files.

/// A link the connector opened toward a destination nobody serves: it stays `Pending`, its
/// request repeated, for the whole establishment timeout.
#[deny(clippy::await_holding_invalid_type)]
async fn errors_pending_link(pair: &LoopbackRequestPair) -> (Arc<Mutex<Link>>, AddressHash) {
    let unserved = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("loopback", "request-errors-unserved"),
    );
    let link = pair.handles.connector.transport.link(unserved.desc).await;
    let link_id = *link.lock().await.id();
    (link, link_id)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_on_a_link_that_is_not_active_is_refused() {
    let pair = loopback_request_pair("request-errors-inactive").await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let (pending_link, pending_link_id) = errors_pending_link(&pair).await;
    assert_ne!(pending_link_id, pair.handles.link_id, "a second link, not the active one");
    assert_eq!(pending_link.lock().await.status(), LinkStatus::Pending);

    let refused = pair
        .handles
        .connector
        .transport
        .request(&pending_link_id, "/echo", rmpv::Value::from("ping"), RequestOptions::default())
        .await;
    assert!(matches!(refused, Err(RnsError::ConnectionError)), "{refused:?}");
    assert_eq!(pending_link.lock().await.status(), LinkStatus::Pending, "refused, not closed");

    // The active link beside it is untouched.
    let receipt = request(&pair, "/echo", rmpv::Value::from("ping")).await;
    let body = await_receipt(&receipt).await.expect("the active link still answers");
    assert_eq!(body.value(), Some(&rmpv::Value::from("ping")));
}
