// Usage-pattern coverage for the oversized-request advertisement warn cadence under mixed
// traffic: the count is a count of refusals alone. An at-limit request advertisement the
// destination accepts and a plain data round trip between the 63rd and the 64th refusal
// leave no refusal record and neither reset nor advance the count, so the 64th refusal
// still warns and the 65th is debug. The acceptance is shown, not assumed: an initiator
// cancel naming the at-limit advertisement draws the `InboundFailed` only a receiver the
// destination created can publish, where the sibling cadence test pins that a refusal
// creates none. Helpers come from the sibling `tests_parts` files included into this
// module.

/// Refusals before the accepted traffic: one short of the first stride's warn.
const REQUEST_ADVERT_REFUSALS_BEFORE: u32 = 63;

/// Refusals after the accepted traffic: the stride's warn and one debug past it.
const REQUEST_ADVERT_REFUSALS_AFTER: u32 = 2;

/// A request advertisement whose advertised data size is exactly `REQUEST_ADVERT_LIMIT`,
/// so the destination's gate (`d > limit`) lets it through.
fn at_limit_request_advertisement(seed: u32) -> ResourceAdvertisement {
    let mut advertisement = over_limit_request_advertisement(seed);
    let data_size = REQUEST_ADVERT_LIMIT as u64;
    advertisement.transfer_size = data_size;
    advertisement.data_size = data_size;
    advertisement
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_accepted_traffic_between_refusals_does_not_move_the_cadence() {
    install_test_logger();
    let mut pair = loopback_link_handles("request-advert-reject-cadence-mixed").await;
    pair.destination
        .lock()
        .await
        .set_max_request_size(REQUEST_ADVERT_LIMIT)
        .expect("the request size limit is accepted");
    let mut listener_resources = pair.listener.transport.resource_events();

    // Seeds past the sibling runs keep a mixed log attributable when they share a logger.
    let base = 5_000;
    for seed in 0..REQUEST_ADVERT_REFUSALS_BEFORE {
        send_over_limit_request_advertisement(&pair.connector.transport, &pair.link, base + seed)
            .await;
    }

    // Accepted traffic: an at-limit request advertisement and a plain data round trip.
    let accepted = at_limit_request_advertisement(base + 10_000);
    let packet = {
        let link = pair.link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceAdvrtisement,
            &accepted.pack().expect("advertisement packs"),
        )
        .expect("advertisement fits a link packet")
    };
    let outcome =
        pair.connector.transport.send_link_packet_on_bound_iface(&pair.link, packet).await;
    assert_eq!(
        outcome,
        SendPacketOutcome::SentDirect,
        "the at-limit advertisement reaches the wire"
    );
    let cancel = {
        let link = pair.link.lock().await;
        crate::resource::build_link_packet(
            &link,
            PacketType::Data,
            PacketContext::ResourceInitiatorCancel,
            accepted.hash.as_slice(),
        )
        .expect("cancel fits a link packet")
    };
    let outcome =
        pair.connector.transport.send_link_packet_on_bound_iface(&pair.link, cancel).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the cancel reaches the wire");
    let failed = await_resource_event(&mut listener_resources, |event| {
        event.link_id == pair.link_id && event.hash == accepted.hash
    })
    .await;
    assert!(
        matches!(
            &failed.kind,
            ResourceEventKind::InboundFailed(failure) if failure.reason == "remote_cancelled"
        ),
        "the at-limit advertisement was accepted into a receiver the cancel tears down, got {:?}",
        failed.kind
    );
    exchange_data_both_ways(&mut pair).await;

    for seed in 0..REQUEST_ADVERT_REFUSALS_AFTER {
        send_over_limit_request_advertisement(
            &pair.connector.transport,
            &pair.link,
            base + REQUEST_ADVERT_REFUSALS_BEFORE + seed,
        )
        .await;
    }
    exchange_data_both_ways(&mut pair).await;

    let records = rejected_request_advertisement_records_for(pair.link_id);
    let total = (REQUEST_ADVERT_REFUSALS_BEFORE + REQUEST_ADVERT_REFUSALS_AFTER) as usize;
    assert_eq!(
        records.len(),
        total,
        "the count is of refusals alone; the accepted advertisement and the data left none, got {}",
        records.len()
    );
    let warned: Vec<u32> = records
        .iter()
        .enumerate()
        .filter(|(_, (level, _))| *level == log::Level::Warn)
        .map(|(index, _)| index as u32 + 1)
        .collect();
    assert_eq!(warned, vec![1, 64], "accepted traffic neither resets nor advances the count");
    let (level, record) = &records[63];
    assert_eq!(*level, log::Level::Warn, "the 64th refusal warns after accepted traffic: {record}");
    assert!(record.ends_with(" rejected=64"), "the 64th refusal carries count 64: {record}");
    let (level, record) = &records[64];
    assert_eq!(*level, log::Level::Debug, "the 65th refusal is debug: {record}");
    assert!(record.ends_with(" rejected=65"), "the 65th refusal carries count 65: {record}");
    assert!(
        rejected_advertisement_records_for(pair.link_id).is_empty(),
        "the over-cap advertisement count stays untouched"
    );
}
