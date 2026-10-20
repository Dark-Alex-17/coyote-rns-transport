// Usage-pattern coverage for a response advertisement the ResourceManager drops: the
// requester's receipt moves to `Receiving` only for an advertisement the manager admitted
// (`RNS/Link.py:1055-1064` touches the receipt only under `if response_resource != None`),
// so one the inbound caps refuse leaves the receipt as it was, `Sent` with no sizes and its
// clock still running, and the request clock then fails it `Timeout`. The link keeps
// answering afterwards. Helpers come from the sibling `tests_parts` files.

/// Builds a response advertisement for `id` the inbound caps refuse: a part count over the
/// 8192-part inbound limit against a few bytes of data.
fn unreasonable_response_advertisement(id: RequestId) -> ResourceAdvertisement {
    let hash = crate::hash::Hash::new_from_rand(OsRng);
    ResourceAdvertisement {
        transfer_size: 64,
        data_size: 64,
        parts: 100_000,
        hash,
        random_hash: [0; crate::resource::RANDOM_HASH_SIZE],
        original_hash: hash,
        segment_index: 1,
        total_segments: 1,
        request_id: Some(serde_bytes::ByteBuf::from(id.to_bytes().to_vec())),
        flags: crate::resource::FLAG_RESPONSE,
        hashmap: Vec::new(),
    }
}

/// Sends `advertisement` from the responder over the pair's link as a resource
/// advertisement packet.
#[deny(clippy::await_holding_invalid_type)]
async fn inject_advertisement_from_listener(
    pair: &LoopbackRequestPair,
    advertisement: &ResourceAdvertisement,
) {
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("the listener tracks the inbound link");
    let packet = {
        let link = in_link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &advertisement.pack().expect("the advertisement packs"),
        )
        .expect("the advertisement fits a link packet")
    };
    let outcome =
        pair.handles.listener.transport.send_link_packet_on_bound_iface(&in_link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the advertisement reaches the wire");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_receipt_dropped_response_advertisement_leaves_the_clock_running(
) {
    let pair = loopback_request_pair("receipt-probe3-dropped-advert").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_timeout(OVERRIDE_TIMEOUT),
    )
    .await;
    let started = receipt.started_at().expect("a packet request starts at the send");
    let mut status_rx = receipt.subscribe();

    let advertisement = unreasonable_response_advertisement(receipt.request_id());
    inject_advertisement_from_listener(&pair, &advertisement).await;

    // Give the advertisement a moment to be handled, then confirm it changed nothing.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(receipt.status(), RequestStatus::Sent, "a refused advertisement is not received");
    assert!(!status_rx.has_changed().expect("the entry is alive"), "no status move was posted");
    assert_eq!(receipt.response_size(), None, "no `d` is recorded for a refused advertisement");
    assert_eq!(receipt.response_transfer_size(), None, "nor a `t`");
    assert_eq!(receipt.progress(), 0.0);

    assert_times_out(&receipt, started).await;
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Failed);

    let after = request(&pair, "/echo", rmpv::Value::from("still here")).await;
    let answer = await_receipt(&after).await.expect("the link still answers");
    assert_eq!(answer.value(), Some(&rmpv::Value::from("still here")));
}
