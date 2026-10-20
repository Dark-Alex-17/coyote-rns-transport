// Loopback coverage for the errors `Transport::request` returns before any request leaves:
// a link the connector knows but that is not `Active` is refused with `ConnectionError`,
// where Python drops the packet only on a `CLOSED` link (`RNS/Packet.py:294-295`). Helpers
// come from the sibling `tests_parts` files.

/// A link the connector opened toward a destination nobody serves: it stays `Pending`, its
/// request repeated, for the whole establishment timeout. The connector does not
/// broadcast, so the unserved destination is given a path over the interface the served
/// one was learned on; without it `Transport::link` refuses the request outright.
#[deny(clippy::await_holding_invalid_type)]
async fn errors_pending_link(pair: &LoopbackRequestPair) -> (Arc<Mutex<Link>>, AddressHash) {
    let unserved = SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("loopback", "request-errors-unserved"),
    );
    let connector = &pair.handles.connector.transport;
    let served_iface = connector
        .path_status(&pair.handles.desc.address_hash)
        .await
        .interface
        .expect("the served destination was learned from its announce");
    assert!(connector.get_handler().lock().await.path_table.restore_tunnel_path(
        unserved.desc.address_hash,
        unserved.desc.address_hash,
        1,
        served_iface,
        Hash::new_from_slice(b"request-errors-unserved"),
        std::time::Instant::now(),
    ));
    let link = connector.link(unserved.desc).await.expect("link request sent");
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
