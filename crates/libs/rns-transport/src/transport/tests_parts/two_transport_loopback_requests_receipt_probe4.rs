// Usage-pattern coverage for a request Resource the requester cancels through
// `Transport::cancel_resource` while it is still on its way out: the receipt fails `Send`
// (`RNS/Link.py:1377-1385` fails the receipt for every Resource status but complete) on every
// clone, with no clock ever started, the connector's `resource_events()` reports
// `OutboundCancelled` for the cancelled hash, a cancel that names nothing in flight leaves the
// receipt as it was, and the link keeps answering afterwards. Helpers come from the sibling
// `tests_parts` files.

/// Reads the request Resource advertisement the connector sent over the pair's link off the
/// listener's interface tap and returns the resource hash it names. The tap fires before any
/// handler lock, so the advertisement is visible while the listener is stalled; the
/// connector's own link decrypts it, the link cipher being symmetric.
#[deny(clippy::await_holding_invalid_type)]
async fn advertised_request_resource_hash(
    pair: &LoopbackRequestPair,
    listener_tap: &mut broadcast::Receiver<RxMessage>,
) -> Hash {
    let message = timeout(LOOPBACK_TIMEOUT, async {
        loop {
            match listener_tap.recv().await {
                Ok(message)
                    if message.packet.destination == pair.handles.link_id
                        && message.packet.context == PacketContext::ResourceAdvrtisement =>
                {
                    return Some(message);
                }
                Ok(_) => continue,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break None,
            }
        }
    })
    .await
    .expect("the request Resource is advertised")
    .expect("the listener tap stays open");
    let mut plain = PacketDataBuffer::new();
    let plain_len = {
        let link = pair.handles.link.lock().await;
        link.decrypt(message.packet.data.as_slice(), plain.accuire_buf_max())
            .expect("the connector's link decrypts its own advertisement")
            .len()
    };
    plain.resize(plain_len);
    let advertisement =
        ResourceAdvertisement::unpack(plain.as_slice()).expect("the advertisement unpacks");
    assert!(advertisement.is_request(), "the connector advertised a request Resource");
    advertisement.original_hash
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_cancelled_request_resource_fails_send() {
    let pair = loopback_request_pair("receipt-probe4-cancel").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let body = over_mdu_body(&pair).await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let mut connector_events = pair.handles.connector.transport.resource_events();
    let connector = &pair.handles.connector.transport;
    let link_id = pair.handles.link_id;

    let stall = stall_transport(&pair.handles.listener.transport).await;
    let receipt = request(&pair, "/quiet", rmpv::Value::Binary(body)).await;
    let clone = receipt.clone();
    let mut status_rx = receipt.subscribe();
    assert_eq!(receipt.status(), RequestStatus::Sent);
    assert_eq!(receipt.started_at(), None, "no clock runs while the request Resource is out");

    let hash = advertised_request_resource_hash(&pair, &mut listener_tap).await;

    // A cancel that names nothing in flight is a no-op for the receipt.
    let unrelated = Hash::new_from_rand(OsRng);
    let cancelled =
        connector.cancel_resource(&link_id, unrelated).await.expect("the link is known");
    assert!(!cancelled, "nothing was cancelled for an unknown hash");
    assert_eq!(receipt.status(), RequestStatus::Sent, "the receipt is untouched");
    assert!(!status_rx.has_changed().expect("the entry is alive"), "no status move was posted");

    let cancelled =
        connector.cancel_resource(&link_id, hash).await.expect("the cancel reaches the wire");
    assert!(cancelled, "the request Resource was still outbound");

    assert_eq!(await_receipt(&receipt).await, Err(RequestError::Send));
    assert_eq!(await_receipt(&clone).await, Err(RequestError::Send), "the clone agrees");
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Failed);
    assert_eq!(receipt.started_at(), None, "the clock never started");
    assert!(receipt.concluded_at().is_some(), "a failed receipt has concluded");
    assert_eq!(receipt.response_size(), None);
    assert_eq!(receipt.response_transfer_size(), None);
    assert_eq!(receipt.progress(), 0.0);

    let event = await_resource_event(&mut connector_events, |event| {
        event.hash == hash && event.link_id == link_id
    })
    .await;
    assert!(
        matches!(event.kind, ResourceEventKind::OutboundCancelled),
        "the broadcast still carries the cancel, got {:?}",
        event.kind
    );

    // A second cancel of the same hash finds nothing to cancel.
    let again = connector.cancel_resource(&link_id, hash).await.expect("the link is still known");
    assert!(!again, "the cancelled Resource is gone from the outbound table");

    stall.await.expect("stall task finished");
    let after = request(&pair, "/echo", rmpv::Value::from("still here")).await;
    let answer = await_receipt(&after).await.expect("the link still answers");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("still here")));
}
