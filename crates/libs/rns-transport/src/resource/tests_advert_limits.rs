/// A split resource advertises each segment with its own `t`, so a 64 MiB
/// total split into segments at or below the reference decode cap must
/// still be admitted even though the whole transfer is far above it.
#[test]
fn resource_manager_admits_first_segment_of_a_large_split_resource() {
    let (mut manager, mut link) = manager_with_test_link();
    let mut first_segment =
        advertisement_with(MAX_EFFICIENT_SIZE as u64, 64 * (MAX_EFFICIENT_SIZE as u64), 1);
    first_segment.segment_index = 1;
    first_segment.total_segments = 64;
    first_segment.flags = FLAG_SPLIT;
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &first_segment.pack().expect("advertisement"),
        *link.id(),
    );

    let responses = manager.handle_packet(&packet, &mut link);
    assert!(!manager.incoming.is_empty(), "first segment of a split resource creates a receiver");
    assert!(!responses.is_empty(), "admitted segment requests parts");
}

/// A rejected over-cap advertisement must leave the manager and the link in a
/// state where the next well-formed advertisement is still admitted.
#[test]
fn resource_manager_keeps_serving_the_link_after_dropping_an_oversized_advertisement() {
    let (mut manager, mut link) = manager_with_test_link();
    let oversized = advertisement_with(MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE + 1, 1, 1);
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &oversized.pack().expect("advertisement"),
        *link.id(),
    );
    assert!(manager.handle_packet(&packet, &mut link).is_empty());
    assert!(manager.incoming.is_empty());

    let mut follow_up = advertisement_with(16, 16, 1);
    follow_up.hash = Hash::new_from_slice(&[4u8; 32]);
    follow_up.original_hash = follow_up.hash;
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &follow_up.pack().expect("advertisement"),
        *link.id(),
    );
    let responses = manager.handle_packet(&packet, &mut link);
    assert!(manager.incoming.contains_key(&follow_up.hash), "follow-up advertisement admitted");
    assert!(!responses.is_empty(), "follow-up advertisement requests parts");
}

/// The decode cap is exactly three times `MAX_EFFICIENT_SIZE`, as in
/// RNS/Resource.py:1374: an advertised `t` at the cap is admitted and `t`
/// one byte above it is dropped without a reply.
#[test]
fn resource_manager_advert_cap_sits_at_three_times_max_efficient_size() {
    let cap = 3 * (MAX_EFFICIENT_SIZE as u64);

    let (mut manager, mut link) = manager_with_test_link();
    let mut over = advertisement_with(cap + 1, cap + 1, 1);
    over.hash = Hash::new_from_slice(&[5u8; 32]);
    over.original_hash = over.hash;
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &over.pack().expect("advertisement"),
        *link.id(),
    );
    assert!(manager.handle_packet(&packet, &mut link).is_empty(), "t = cap + 1 must not reply");
    assert!(!manager.incoming.contains_key(&over.hash), "t = cap + 1 must not create a receiver");

    let mut at_cap = advertisement_with(cap, cap, 1);
    at_cap.hash = Hash::new_from_slice(&[6u8; 32]);
    at_cap.original_hash = at_cap.hash;
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &at_cap.pack().expect("advertisement"),
        *link.id(),
    );
    let responses = manager.handle_packet(&packet, &mut link);
    assert!(manager.incoming.contains_key(&at_cap.hash), "t = cap creates a receiver");
    assert!(!responses.is_empty(), "t = cap requests parts");
}
