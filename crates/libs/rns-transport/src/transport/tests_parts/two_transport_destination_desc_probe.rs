// Usage-pattern coverage for `Transport::destination_desc` beyond the lookup itself: the
// desc the connector gets back is usable as the argument of `Transport::link`, its name
// is the zero-padded truncated hash, neither accessor self-learns on the announcing
// side, and `deregister_destination` is the one call that prunes it.
// Helpers come from the sibling `tests_parts` files included into this module.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_opens_a_link_and_agrees_with_destination_identity_on_both_sides() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "destination-desc-link").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;

    // The announcing side does not learn its own destination: it lives in the input map,
    // not the announced map both accessors read.
    assert!(
        listener.transport.destination_desc(&announced.address_hash).await.is_none(),
        "the announcer has no desc entry for its own destination"
    );
    assert!(
        listener.transport.destination_identity(&announced.address_hash).await.is_none(),
        "the announcer has no identity entry for its own destination"
    );

    let desc = connector
        .transport
        .destination_desc(&announced.address_hash)
        .await
        .expect("the connector learned the destination");
    let name_hash = desc.name.hash.as_slice();
    assert_eq!(
        desc.name.as_name_hash_slice().len(),
        crate::destination::NAME_HASH_LENGTH,
        "the comparable name is the truncated hash"
    );
    assert!(
        name_hash[crate::destination::NAME_HASH_LENGTH..].iter().all(|byte| *byte == 0),
        "the stored name is the truncated hash zero-padded to a full hash: {name_hash:02x?}"
    );

    // The desc the accessor returns is the argument `Transport::link` takes; the link it
    // opens activates on both ends under one id.
    let (_link, link_id) = open_link(&connector.transport, desc).await;
    let activated =
        await_event(&mut in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn destination_desc_is_pruned_by_deregister_destination_together_with_the_identity() {
    let listener = listener("loopback-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("loopback-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;

    let (_destination, announced) =
        register_in_destination(&listener.transport, "loopback", "destination-desc-prune").await;
    learn_announce(&mut connector.announces, &announced.address_hash).await;
    assert!(connector.transport.destination_desc(&announced.address_hash).await.is_some());

    // Expiring the path leaves the entry; deregistering removes it, and the identity
    // accessor and the path table report the same state before and after.
    assert!(connector.transport.expire_path(&announced.address_hash).await, "the path existed");
    assert!(
        connector.transport.deregister_destination(&announced.address_hash).await,
        "deregistering a destination learned from an announce removes it"
    );
    assert!(connector.transport.destination_desc(&announced.address_hash).await.is_none());
    assert!(connector.transport.destination_identity(&announced.address_hash).await.is_none());
    assert!(!connector.transport.has_path(&announced.address_hash).await);
    assert!(
        !connector.transport.deregister_destination(&announced.address_hash).await,
        "a second deregistration finds nothing to remove"
    );
}
