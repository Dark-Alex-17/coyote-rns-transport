// Usage-pattern coverage for the oversized-request advertisement warn cadence past its
// first stride: `request_advert_reject_warn_cadence.rs` stops at the 65th refusal, where a
// cadence that warned only on the first and the 64th would still pass. Here the run crosses
// the second stride, so the 128th refusal warns, the 65th through 127th and the 129th are
// debug, and the warn records are exactly three. Helpers come from the sibling
// `tests_parts` files included into this module.

/// One past the second stride, so the run shows the 128th warn and a debug after it.
const REQUEST_ADVERT_REJECTIONS_TWO_STRIDES: u32 = 129;

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_warns_again_on_the_128th_rejected_request_advertisement() {
    install_test_logger();
    let mut pair = loopback_link_handles("request-advert-reject-cadence-two-strides").await;
    pair.destination
        .lock()
        .await
        .set_max_request_size(REQUEST_ADVERT_LIMIT)
        .expect("the request size limit is accepted");

    for seed in 0..REQUEST_ADVERT_REJECTIONS_TWO_STRIDES {
        // Seeds start past the sibling test's range so the hashes never collide with its
        // advertisements should both run against one logger.
        send_over_limit_request_advertisement(&pair.connector.transport, &pair.link, 1_000 + seed)
            .await;
    }
    exchange_data_both_ways(&mut pair).await;

    let records = rejected_request_advertisement_records_for(pair.link_id);
    assert_eq!(
        records.len(),
        REQUEST_ADVERT_REJECTIONS_TWO_STRIDES as usize,
        "every refusal leaves a record, got {}",
        records.len()
    );
    let warned: Vec<u32> = records
        .iter()
        .enumerate()
        .filter(|(_, (level, _))| *level == log::Level::Warn)
        .map(|(index, _)| index as u32 + 1)
        .collect();
    assert_eq!(warned, vec![1, 64, 128], "warn on the first and on every 64th refusal");
    for (index, (level, record)) in records.iter().enumerate() {
        let count = index as u32 + 1;
        if !warned.contains(&count) {
            assert_eq!(*level, log::Level::Debug, "refusal {count} is a debug record: {record}");
        }
        assert!(
            record.contains(&format!(" rejected={count}")),
            "refusal {count} carries its running count: {record}"
        );
    }
    assert!(
        rejected_advertisement_records_for(pair.link_id).is_empty(),
        "the over-cap advertisement count stays untouched"
    );
}
