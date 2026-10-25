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

/// Every over-cap advertisement a link sends is counted against that link,
/// so the warn cadence can be derived from the count rather than logged on
/// every drop.
#[test]
fn resource_manager_counts_rejected_advertisements_per_link() {
    let (mut manager, mut link) = manager_with_test_link();
    for seed in [7u8, 8u8] {
        let mut oversized = advertisement_with(MAX_REFERENCE_ADVERTISED_TRANSFER_SIZE + 1, 1, 1);
        oversized.hash = Hash::new_from_slice(&[seed; 32]);
        oversized.original_hash = oversized.hash;
        let packet = resource_packet(
            PacketContext::ResourceAdvrtisement,
            &oversized.pack().expect("advertisement"),
            *link.id(),
        );
        assert!(manager.handle_packet(&packet, &mut link).is_empty());
    }
    assert!(manager.incoming.is_empty());
    assert_eq!(manager.link_stats[link.id()].rejected_advertisements, 2);
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

/// The advertised `d` is bounded on its own: a small `t` does not get a
/// receiver past the 64 MiB inbound data cap, and `d` exactly at the cap is
/// still admitted.
#[test]
fn resource_receiver_refuses_advertised_data_size_over_the_inbound_cap() {
    let link_id = AddressHash::new_from_slice(&[9u8; 32]);
    assert_eq!(MAX_INBOUND_RESOURCE_TRANSFER_SIZE, 64 * 1024 * 1024);

    let over = advertisement_with(1, MAX_INBOUND_RESOURCE_TRANSFER_SIZE + 1, 1);
    assert!(matches!(
        ResourceReceiver::new_with_mtu(&over, link_id, DEFAULT_RESOURCE_INTERFACE_MTU),
        Err(RnsError::InvalidArgument)
    ));

    let at_cap = advertisement_with(1, MAX_INBOUND_RESOURCE_TRANSFER_SIZE, 1);
    assert!(
        ResourceReceiver::new_with_mtu(&at_cap, link_id, DEFAULT_RESOURCE_INTERFACE_MTU).is_ok(),
        "d = cap creates a receiver"
    );
}

/// An advertisement the receiver constructor refuses, here `d` one byte over the
/// inbound data cap, is charged to the link like one the manager's own guards drop,
/// so it shares the same warn cadence instead of warning on every advertisement.
#[test]
fn resource_manager_counts_an_advertisement_the_receiver_refuses() {
    let (mut manager, mut link) = manager_with_test_link();
    let over = advertisement_with(1, MAX_INBOUND_RESOURCE_TRANSFER_SIZE + 1, 1);
    let packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &over.pack().expect("advertisement"),
        *link.id(),
    );
    assert!(manager.handle_packet(&packet, &mut link).is_empty());
    assert!(manager.incoming.is_empty(), "a refused advertisement creates no receiver");
    assert_eq!(manager.link_stats[link.id()].rejected_advertisements, 1);
}
