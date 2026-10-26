// The two link-scoped teardown arms the other cross-link cases do not drive: a receiver on
// one link whose hash aliases another link's split assembly fails by itself, through the
// retry sweep and through the local response cancel, and in both cases the assembly it
// named survives and goes on to complete.

/// Link a has segment 1 of a two-segment resource assembled and nothing in flight; link b
/// is admitted a one-segment resource whose hash is link a's original hash and then
/// exhausts its retry budget. The failure is reported under link b with its own progress,
/// link a's assembly stays, and link a's segment 2 still completes the resource.
#[test]
fn resource_manager_retry_exhaustion_on_another_link_keeps_the_split_assembly() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
    let (original_hash, second_adv, second_part, expected) =
        link_a_has_segment_one(&mut manager, &mut link_a);
    admit_forged_one_segment_receiver(&mut manager, &mut link_b, original_hash);

    manager.retry_requests(Instant::now() + Duration::from_secs(2));

    assert!(!manager.incoming.contains_key(&original_hash), "link b's receiver is gone");
    assert!(manager.incoming_segments.contains_key(&original_hash), "link a's assembly stays");
    let events = manager.drain_events();
    assert_eq!(events.len(), 1, "only link b's receiver is reported: {events:?}");
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link_b.id(), "the failure is link b's");
    let ResourceEventKind::InboundFailed(failure) = &events[0].kind else {
        panic!("expected an inbound failure, got {:?}", events[0].kind);
    };
    assert_eq!(failure.reason, "retry_limit_exhausted");
    assert_eq!(failure.progress.received_parts, 0, "link b's own progress, not link a's assembly");

    assert_link_a_completes(&mut manager, &mut link_a, &second_adv, &second_part, &expected);
}

/// The same alias, torn down by the local response cancel on link b: the live receiver
/// under the hash belongs to link b, so it goes and is handed back as link b's
/// `cancelled`, while link a's assembly under the same hash is left to complete.
#[test]
fn resource_manager_cancel_incoming_response_on_another_link_keeps_the_split_assembly() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
    let (original_hash, second_adv, second_part, expected) =
        link_a_has_segment_one(&mut manager, &mut link_a);
    admit_forged_one_segment_receiver(&mut manager, &mut link_b, original_hash);

    let cancelled = manager.cancel_incoming_response(original_hash, original_hash, link_b.id());

    assert!(cancelled.receiver_in_flight, "link b's receiver was live");
    let event = cancelled.event.expect("link b's cancel is handed back");
    assert_eq!(event.hash, original_hash);
    assert_eq!(event.link_id, *link_b.id(), "the cancel is link b's");
    let ResourceEventKind::InboundFailed(failure) = &event.kind else {
        panic!("expected an inbound failure, got {:?}", event.kind);
    };
    assert_eq!(failure.reason, "cancelled");
    assert_eq!(failure.progress.received_parts, 0, "link b's own progress, not link a's assembly");
    assert!(!manager.incoming.contains_key(&original_hash), "link b's receiver is gone");
    assert!(manager.incoming_segments.contains_key(&original_hash), "link a's assembly stays");
    assert!(manager.drain_events().is_empty(), "nothing is queued for link a");

    assert_link_a_completes(&mut manager, &mut link_a, &second_adv, &second_part, &expected);
}

/// Advertises and delivers segment 1 of 2 on `link_a`, leaving the assembly waiting for
/// segment 2. Returns the original hash, segment 2's advertisement and part, and the data
/// the completed resource must carry.
fn link_a_has_segment_one(
    manager: &mut ResourceManager,
    link_a: &mut Link,
) -> (Hash, ResourceAdvertisement, Vec<u8>, Vec<u8>) {
    let first_data = b"first-segment";
    let second_data = b"second-segment";
    let total = (first_data.len() + second_data.len()) as u64;
    let (first_adv, first_part) = split_test_segment(first_data, None, 1, 2, total);
    let original_hash = first_adv.hash;
    let first_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &first_adv.pack().expect("first advertisement"),
        *link_a.id(),
    );
    assert_eq!(manager.handle_packet(&first_packet, link_a).len(), 1);
    assert_eq!(
        manager
            .handle_packet(
                &resource_packet(PacketContext::Resource, &first_part, *link_a.id()),
                link_a
            )
            .len(),
        1,
        "the first segment is proven on link a"
    );
    manager.drain_events();
    assert!(manager.incoming_segments.contains_key(&original_hash), "segment 1 is assembled");
    assert!(manager.incoming.is_empty(), "nothing is in flight on link a");

    let (second_adv, second_part) =
        split_test_segment(second_data, Some(original_hash), 2, 2, total);
    (
        original_hash,
        second_adv,
        second_part,
        [first_data.as_slice(), second_data.as_slice()].concat(),
    )
}

/// Admits on `link_b` a one-segment resource whose hash is `original_hash`, as the
/// existing initiator-cancel case does.
fn admit_forged_one_segment_receiver(
    manager: &mut ResourceManager,
    link_b: &mut Link,
    original_hash: Hash,
) {
    let forged = ResourceAdvertisement {
        transfer_size: 1,
        data_size: 1,
        parts: 1,
        hash: original_hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash,
        segment_index: 1,
        total_segments: 1,
        request_id: None,
        flags: 0,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    let forged_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &forged.pack().expect("forged advertisement"),
        *link_b.id(),
    );
    assert_eq!(manager.handle_packet(&forged_packet, link_b).len(), 1, "admitted on link b");
    manager.drain_events();
    assert!(manager.incoming.contains_key(&original_hash), "link b's receiver is in flight");
}

/// Delivers segment 2 on `link_a` and expects the assembled resource under link a.
fn assert_link_a_completes(
    manager: &mut ResourceManager,
    link_a: &mut Link,
    second_adv: &ResourceAdvertisement,
    second_part: &[u8],
    expected: &[u8],
) {
    let second_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &second_adv.pack().expect("second advertisement"),
        *link_a.id(),
    );
    assert_eq!(manager.handle_packet(&second_packet, link_a).len(), 1, "segment 2 is requested");
    assert_eq!(
        manager
            .handle_packet(
                &resource_packet(PacketContext::Resource, second_part, *link_a.id()),
                link_a
            )
            .len(),
        1,
        "the second segment is proven on link a"
    );
    let events = manager.drain_events();
    let complete = events
        .iter()
        .find(|event| matches!(event.kind, ResourceEventKind::Complete(_)))
        .unwrap_or_else(|| panic!("link a's resource completes: {events:?}"));
    assert_eq!(complete.hash, second_adv.original_hash);
    assert_eq!(complete.link_id, *link_a.id());
    let ResourceEventKind::Complete(done) = &complete.kind else { unreachable!() };
    assert_eq!(done.data, expected, "both segments assembled in order on link a");
    assert!(
        !manager.incoming_segments.contains_key(&second_adv.original_hash),
        "assembly consumed"
    );
}
