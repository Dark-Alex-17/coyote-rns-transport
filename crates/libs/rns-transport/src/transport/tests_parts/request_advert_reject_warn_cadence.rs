// Usage-pattern coverage for the oversized-request advertisement warn cadence, read from
// the wire side: a peer that keeps advertising request resources above the destination's
// `max_request_size` (`RNS/Link.py:1036-1046`) is warned about on its first refusal and on
// every 64th after that, every other refusal is a debug record, the count is kept per link
// and apart from the over-cap advertisement count. Helpers come from the sibling
// `tests_parts` files; the records come from the binary's test logger.

/// One more than the stride, so the run crosses the second warn exactly once.
const REQUEST_ADVERT_REJECTIONS_SENT: u32 = 65;

/// Small enough that the advertisement below is refused on its advertised data size alone.
const REQUEST_ADVERT_LIMIT: usize = 256;

/// A request advertisement one byte over `REQUEST_ADVERT_LIMIT`, with a hash and request
/// id derived from `seed` so each one is a distinct request.
fn over_limit_request_advertisement(seed: u32) -> ResourceAdvertisement {
    let mut hash = [0u8; 32];
    hash[..4].copy_from_slice(&seed.to_be_bytes());
    hash[4] = 0x5a;
    let hash = Hash::new_from_slice(&hash);
    let data_size = REQUEST_ADVERT_LIMIT as u64 + 1;
    ResourceAdvertisement {
        transfer_size: data_size,
        data_size,
        parts: 1,
        hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: hash,
        segment_index: 1,
        total_segments: 1,
        request_id: Some(serde_bytes::ByteBuf::from(hash.as_slice()[..ADDRESS_HASH_SIZE].to_vec())),
        flags: crate::resource::FLAG_REQUEST,
        hashmap: vec![0u8; MAPHASH_LEN],
    }
}

/// Sends `over_limit_request_advertisement(seed)` over `link` and checks it reached the
/// wire.
#[deny(clippy::await_holding_invalid_type)]
async fn send_over_limit_request_advertisement(
    transport: &Transport,
    link: &Arc<Mutex<Link>>,
    seed: u32,
) {
    let packet = {
        let link = link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &over_limit_request_advertisement(seed).pack().expect("advertisement packs"),
        )
        .expect("advertisement fits a link packet")
    };
    let outcome = transport.send_link_packet_on_bound_iface(link, packet).await;
    assert_eq!(
        outcome,
        SendPacketOutcome::SentDirect,
        "request advertisement {seed} reaches the wire"
    );
}

/// 65 over-limit request advertisements on one link produce 65 records, the first and the
/// 64th at warn and every other at debug, each naming the link and the running count; a
/// second link to the same transport starts its own count and is warned on its first
/// refusal even though the transport has already refused 65; and none of them is charged to
/// the over-cap advertisement count.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_warns_on_the_first_and_every_64th_rejected_request_advertisement_per_link(
) {
    install_test_logger();
    let mut pair = loopback_link_handles("request-advert-reject-cadence").await;
    pair.destination
        .lock()
        .await
        .set_max_request_size(REQUEST_ADVERT_LIMIT)
        .expect("the request size limit is accepted");
    let mut listener_resources = pair.listener.transport.resource_events();

    for seed in 0..REQUEST_ADVERT_REJECTIONS_SENT {
        send_over_limit_request_advertisement(&pair.connector.transport, &pair.link, seed).await;
    }
    // The ping travels the same stream after the advertisements, so once it is observed
    // every advertisement before it has been processed and refused.
    exchange_data_both_ways(&mut pair).await;

    let records = rejected_request_advertisement_records_for(pair.link_id);
    assert_eq!(
        records.len(),
        REQUEST_ADVERT_REJECTIONS_SENT as usize,
        "every refusal leaves a record, got {records:?}"
    );
    for (index, (level, record)) in records.iter().enumerate() {
        let count = index as u32 + 1;
        let expected = if count == 1 || count == 64 { log::Level::Warn } else { log::Level::Debug };
        assert_eq!(*level, expected, "refusal {count} is logged at {expected}: {record}");
        assert!(
            record.ends_with(&format!(" rejected={count}")),
            "refusal {count} carries its running count: {record}"
        );
        assert!(
            record.contains(&format!("limit={REQUEST_ADVERT_LIMIT}")),
            "the record names the destination's limit: {record}"
        );
    }
    assert!(
        rejected_advertisement_records_for(pair.link_id).is_empty(),
        "an oversized request is not counted as an over-cap advertisement"
    );
    loop {
        match listener_resources.try_recv() {
            Ok(event) => assert_ne!(
                event.link_id, pair.link_id,
                "a refused request advertisement must not create receiver state: {:?}",
                event.kind
            ),
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => break,
        }
    }

    let mut second =
        connector("loopback-connector-2", pair.listener.port, TcpClient::DEFAULT_MTU).await;
    pair.listener.await_accepted_connections(2).await;
    let announce =
        pair.destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    pair.listener.transport.send_packet(announce).await;
    let learned = learn_announce(&mut second.announces, &pair.desc.address_hash).await;
    let (second_link, second_link_id) = open_link(&second.transport, learned).await;
    await_event(&mut pair.in_events, |event| {
        event.id == second_link_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    assert_ne!(second_link_id, pair.link_id, "the second connector opens a distinct link");

    send_over_limit_request_advertisement(
        &second.transport,
        &second_link,
        REQUEST_ADVERT_REJECTIONS_SENT,
    )
    .await;
    let report = second
        .transport
        .send_to_out_links_with_report(
            &pair.desc.address_hash,
            b"ping after request advert on link two",
        )
        .await;
    assert_sent_on_one_link(report, "second outbound");
    await_event(&mut pair.in_events, |event| {
        event.id == second_link_id && is_plain_data(&event.event)
    })
    .await;

    let second_records = rejected_request_advertisement_records_for(second_link_id);
    assert_eq!(
        second_records.len(),
        1,
        "the second link's first refusal is recorded on its own count, got {second_records:?}"
    );
    assert_eq!(
        second_records[0].0,
        log::Level::Warn,
        "the second link's first refusal warns: {}",
        second_records[0].1
    );
    assert!(
        second_records[0].1.ends_with(" rejected=1"),
        "the count is per link, not per transport: {}",
        second_records[0].1
    );
    assert_eq!(
        rejected_request_advertisement_records_for(pair.link_id).len(),
        REQUEST_ADVERT_REJECTIONS_SENT as usize,
        "the second link's refusal is not charged to the first link"
    );
}
