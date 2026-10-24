// Coverage for the two edges of `send_resource_retry`: a retry whose link interface is not
// attached records that it reached nothing, and a retry for a link without a bound
// interface still leaves through the handler's path routing. Both drive the function
// directly on the connector's handler of a loopback pair, holding the handler guard across
// the call as the retry worker does. Helpers come from the sibling `tests_parts` files.

#[tokio::test]
async fn resource_retry_on_an_unknown_iface_records_that_it_was_not_dispatched() {
    install_test_logger();
    let pair = loopback_link_handles("resource-retry-unknown-iface").await;
    let link_id = pair.link_id;
    let packet = pair.link.lock().await.data_packet(b"retry nowhere").expect("data packet");
    let unknown_iface = AddressHash::new_from_rand(OsRng);

    {
        let handler = pair.connector.transport.get_handler();
        let mut handler = handler.lock().await;
        super::jobs::send_resource_retry(&mut handler, Some(unknown_iface), packet).await;
    }

    let records = resource_diag_records_for(link_id);
    assert_eq!(records.len(), 1, "one diagnostic for the retry: {records:?}");
    assert!(
        records[0].contains("retry_not_dispatched")
            && records[0].contains(&unknown_iface.to_string()),
        "the diagnostic names the interface that matched nothing: {records:?}"
    );
}

#[tokio::test]
async fn resource_retry_without_a_link_iface_leaves_through_path_routing() {
    install_test_logger();
    let mut pair = loopback_link_handles("resource-retry-no-iface").await;
    let link_id = pair.link_id;
    let iface = pair.link.lock().await.ingress_iface().expect("the initiator's link is bound");
    let packet = pair.link.lock().await.data_packet(b"retry via path").expect("data packet");

    {
        let handler = pair.connector.transport.get_handler();
        let mut handler = handler.lock().await;
        // The pair runs with broadcast off, so path routing only reaches the listener
        // through an entry for the link id.
        assert!(handler.path_table.restore_tunnel_path(
            link_id,
            link_id,
            1,
            iface,
            packet.hash(),
            std::time::Instant::now(),
        ));
        super::jobs::send_resource_retry(&mut handler, None, packet).await;
    }

    let delivered = await_event(&mut pair.in_events, |event| {
        event.id == link_id && is_plain_data(&event.event)
    })
    .await;
    assert_eq!(data_bytes(delivered), b"retry via path");
    let records = resource_diag_records_for(link_id);
    assert_eq!(records.len(), 1, "one diagnostic for the retry: {records:?}");
    assert!(
        records[0].contains("retry_without_link_iface"),
        "the diagnostic names the fallback taken: {records:?}"
    );
}
