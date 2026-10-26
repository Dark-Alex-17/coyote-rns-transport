// Usage-pattern coverage for the over-cap advertisement warn cadence, read from the wire
// side: a peer that keeps offering resources above the reference transfer cap
// (`RNS/Resource.py:1374`) is warned about on its first rejection and on every 64th after
// that, the count is kept per link, and no rejection creates receiver state. Helpers come
// from the sibling `tests_parts` files; the records come from the binary's test logger.

/// One more than the stride, so the run crosses the second warn exactly once.
const ADVERT_REJECTIONS_SENT: u32 = 65;

/// An advertisement one byte over the reference transfer cap, with a hash derived from
/// `seed` so each one is a distinct resource.
fn over_cap_advertisement(seed: u32) -> ResourceAdvertisement {
    let mut hash = [0u8; 32];
    hash[..4].copy_from_slice(&seed.to_be_bytes());
    hash[4] = 0xa5;
    let hash = Hash::new_from_slice(&hash);
    ResourceAdvertisement {
        transfer_size: crate::resource::MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE + 1,
        data_size: 1,
        parts: 1,
        hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: hash,
        segment_index: 1,
        total_segments: 1,
        request_id: None,
        flags: 0,
        hashmap: vec![0u8; MAPHASH_LEN],
    }
}

/// Sends `over_cap_advertisement(seed)` over `link` and checks it reached the wire.
#[deny(clippy::await_holding_invalid_type)]
async fn send_over_cap_advertisement(transport: &Transport, link: &Arc<Mutex<Link>>, seed: u32) {
    let packet = {
        let link = link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &over_cap_advertisement(seed).pack().expect("advertisement packs"),
        )
        .expect("advertisement fits a link packet")
    };
    let outcome = transport.send_link_packet_on_bound_iface(link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "advertisement {seed} reaches the wire");
}

/// 65 over-cap advertisements on one link produce exactly two warnings, the first and the
/// 64th rejection, both naming the link and the running count; a second link to the same
/// transport starts its own count and is warned on its first rejection even though the
/// transport has already rejected 65.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_warns_on_the_first_and_every_64th_rejected_advertisement_per_link()
{
    install_test_logger();
    let mut pair = loopback_link_handles("advert-reject-cadence").await;
    let mut listener_resources = pair.listener.transport.resource_events();

    for seed in 0..ADVERT_REJECTIONS_SENT {
        send_over_cap_advertisement(&pair.connector.transport, &pair.link, seed).await;
    }
    // The ping travels the same stream after the advertisements, so once it is observed
    // every advertisement before it has been processed and rejected.
    exchange_data_both_ways(&mut pair).await;

    let records = rejected_advertisement_records_for(pair.link_id);
    assert_eq!(
        records.len(),
        2,
        "65 rejections warn on the first and the 64th only, got {records:?}"
    );
    assert!(
        records.iter().all(|(level, _)| *level == log::Level::Warn),
        "a rejected over-cap peer is visible at warn level, got {records:?}"
    );
    assert!(records[0].1.contains("rejected=1"), "first warn carries count 1: {}", records[0].1);
    assert!(records[1].1.contains("rejected=64"), "second warn carries count 64: {}", records[1].1);
    assert!(
        records.iter().all(|(_, record)| record.contains("transfer_size=")),
        "the warn names the advertised size: {records:?}"
    );
    loop {
        match listener_resources.try_recv() {
            Ok(event) => assert_ne!(
                event.link_id, pair.link_id,
                "a rejected advertisement must not create receiver state: {:?}",
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

    send_over_cap_advertisement(&second.transport, &second_link, ADVERT_REJECTIONS_SENT).await;
    let report = second
        .transport
        .send_to_out_links_with_report(&pair.desc.address_hash, b"ping after advert on link two")
        .await;
    assert_sent_on_one_link(report, "second outbound");
    await_event(&mut pair.in_events, |event| {
        event.id == second_link_id && is_plain_data(&event.event)
    })
    .await;

    let second_records = rejected_advertisement_records_for(second_link_id);
    assert_eq!(
        second_records.len(),
        1,
        "the second link's first rejection warns on its own count, got {second_records:?}"
    );
    assert!(
        second_records[0].1.contains("rejected=1"),
        "the count is per link, not per transport: {}",
        second_records[0].1
    );
    assert_eq!(
        rejected_advertisement_records_for(pair.link_id).len(),
        2,
        "the second link's rejection is not charged to the first link"
    );
}
