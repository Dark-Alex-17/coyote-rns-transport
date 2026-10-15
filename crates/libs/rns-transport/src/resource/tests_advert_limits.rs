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
