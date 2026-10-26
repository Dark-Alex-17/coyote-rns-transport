fn multi_part_sender() -> ResourceSender {
    let (_, link) = manager_with_test_link();
    let mut payload = vec![0u8; 4 * PACKET_MDU];
    OsRng.fill_bytes(&mut payload);
    let sender = ResourceSender::new(&link, payload, None).expect("resource sender");
    assert!(sender.parts.len() > 1, "the sender needs more than one part");
    sender
}

/// A part served before the advertisement was confirmed keeps its serve-time
/// timestamps: only the advertisement clock and the retry budget move.
#[test]
fn confirm_advertised_keeps_serve_timestamps_once_a_part_was_sent() {
    let mut sender = multi_part_sender();
    sender.sent_parts[0] = true;
    let last_activity = sender.last_activity;
    let last_part_sent = sender.last_part_sent;
    let adv_sent = sender.adv_sent;
    std::thread::sleep(Duration::from_millis(5));

    sender.confirm_advertised(16);

    assert_eq!(sender.last_activity, last_activity);
    assert_eq!(sender.last_part_sent, last_part_sent);
    assert!(sender.adv_sent > adv_sent, "the advertisement clock still moves");
    assert_eq!(sender.status, ResourceStatus::Transferring);
    assert_eq!(sender.max_retries, 16);
    assert_eq!(sender.retries_left, 16);
}

/// Without a served part the confirmation is a plain advertisement: every
/// clock moves and the retry budget is the advertisement one.
#[test]
fn confirm_advertised_without_served_parts_behaves_like_mark_advertised() {
    let mut sender = multi_part_sender();
    let last_activity = sender.last_activity;
    std::thread::sleep(Duration::from_millis(5));

    sender.confirm_advertised(16);

    assert!(sender.last_activity > last_activity);
    assert_eq!(sender.last_part_sent, sender.last_activity);
    assert_eq!(sender.adv_sent, sender.last_activity);
    assert_eq!(sender.status, ResourceStatus::Advertised);
    assert_eq!(sender.retries_left, DEFAULT_RESOURCE_MAX_ADV_RETRIES);
}

/// With every part already served the sender goes straight to awaiting the
/// proof, and its retry budget is clamped to the bounded proof wait.
#[test]
fn confirm_advertised_with_every_part_sent_clamps_the_proof_retry_budget() {
    let mut sender = multi_part_sender();
    sender.sent_parts.fill(true);

    sender.confirm_advertised(16);

    assert_eq!(sender.status, ResourceStatus::AwaitingProof);
    assert_eq!(sender.max_retries, 16);
    assert_eq!(sender.retries_left, MAX_AWAITING_PROOF_RETRIES);

    let mut sender = multi_part_sender();
    sender.sent_parts.fill(true);
    sender.confirm_advertised(0);
    assert_eq!(sender.retries_left, 1, "a zero budget still waits one interval for the proof");
}
