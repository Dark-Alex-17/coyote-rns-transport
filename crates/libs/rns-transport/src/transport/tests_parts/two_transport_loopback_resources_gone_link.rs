// Loopback coverage for the resource-retry worker when the link an outbound Resource was
// advertised on disappears before its retry is due. The retry addresses a Link, so without
// one it has nowhere to go: nothing may leave the sender towards the peer, the transfer must
// still reach its terminal event instead of hanging, and the worker that skipped the retry
// must keep serving the transport. Helpers come from the sibling `tests_parts` files.

/// Polls the tap until `want` packets for `link_id` with `context` have been read off the
/// wire, so the test can order its own steps after the first advertisement really landed.
#[deny(clippy::await_holding_invalid_type)]
async fn await_link_packets(
    tap: &mut broadcast::Receiver<RxMessage>,
    link_id: AddressHash,
    context: PacketContext,
    want: usize,
) -> usize {
    let deadline = Instant::now() + LOOPBACK_TIMEOUT;
    let mut seen = 0;
    while seen < want {
        seen += count_link_packets(tap, link_id, context);
        if seen >= want {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {want} {context:?} packet(s) on link {link_id}; saw {seen}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    seen
}

/// Drops every initiator-side link from `transport` and returns how many were dropped. The
/// guard is released before the function returns, so nothing awaits while it is held.
#[deny(clippy::await_holding_invalid_type)]
async fn forget_out_links(transport: &Transport) -> usize {
    let handler = transport.get_handler();
    let mut handler = handler.lock().await;
    let removed = handler.out_links.len();
    handler.out_links.clear();
    removed
}

/// Advertisement retry whose link is gone. The listener is stalled so it cannot answer; the
/// connector advertises a small Resource and forgets the link as soon as the dispatch
/// returns. `send_resource` hands the advertisement to the interface and starts the retry
/// clock under the handler lock it releases just before returning, and the retry worker
/// needs that same lock and only re-advertises once the retry interval has elapsed, so the
/// link is gone before a retry can be due; waiting for the advertisement to be read off the
/// listener's wire first would leave the drop racing the worker on a slow host. From then
/// on every retry the worker pulls has no link: the listener must see exactly the one
/// advertisement that was dispatched while the link existed, the sender must still report
/// `OutboundFailed` once its retry budget runs out (so the worker kept polling after the
/// skip), and a fresh link over the same two transports must still carry a Resource end to
/// end (so the worker did not stop on the skipped retry). The listener's own request for
/// the orphaned advertisement is dropped by the connector, which no longer knows the link.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_advertisement_retry_skips_gone_link() {
    let mut pair = loopback_link_pair("adv-gone").await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut listener_tap = pair.listener.transport.iface_rx();

    let stall = stall_transport(&pair.listener.transport).await;
    let payload = random_payload(SMALL_RESOURCE_LEN);
    let resource_hash = pair
        .connector
        .transport
        .send_resource(&pair.link_id, payload, None)
        .await
        .expect("resource dispatched from the initiator side");
    let removed = forget_out_links(&pair.connector.transport).await;
    assert_eq!(removed, 1, "the connector held exactly the one link under test");

    let first =
        await_link_packets(&mut listener_tap, pair.link_id, PacketContext::ResourceAdvrtisement, 1)
            .await;
    assert_eq!(
        first, 1,
        "only the advertisement dispatched while the link existed reaches the listener"
    );
    stall.await.expect("stall task finished");

    let outbound = await_outbound_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(outbound, ResourceEventKind::OutboundFailed),
        "a Resource whose link is gone must fail once its retries are spent, got {outbound:?}"
    );

    let stray =
        count_link_packets(&mut listener_tap, pair.link_id, PacketContext::ResourceAdvrtisement);
    assert_eq!(
        stray, 0,
        "no advertisement retry may reach the listener after the link is gone; saw {stray}"
    );

    // The same two transports must still carry a Resource over a new link.
    let mut in_events = pair.listener.transport.in_link_events();
    let (_destination, desc) =
        register_in_destination(&pair.listener.transport, "loopback", "adv-gone-fresh").await;
    let learned = learn_announce(&mut pair.connector.announces, &desc.address_hash).await;
    let (_link, fresh_link_id) = open_link(&pair.connector.transport, learned).await;
    await_event(&mut in_events, |event| {
        event.id == fresh_link_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_ne!(fresh_link_id, pair.link_id, "the fresh link has its own id");

    let mut receiver_events = pair.listener.transport.resource_events();
    let payload = random_payload(SMALL_RESOURCE_LEN);
    let resource_hash = pair
        .connector
        .transport
        .send_resource(&fresh_link_id, payload.clone(), None)
        .await
        .expect("resource dispatched on the fresh link");
    assert_transfer_completes(
        &mut sender_events,
        &mut receiver_events,
        fresh_link_id,
        resource_hash,
        &payload,
        None,
    )
    .await;
    assert_no_further_complete(&mut receiver_events, fresh_link_id).await;
}
