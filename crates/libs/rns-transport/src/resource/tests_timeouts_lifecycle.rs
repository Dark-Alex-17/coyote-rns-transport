#[test]
fn resource_sender_emits_outbound_failed_when_status_is_failed() {
    // Covers the new `ResourceStatus::Failed => OutboundResourcePoll::Failed` arm
    // in poll().  The same path is exercised when handle_request_into() fails to
    // build a packet and sets self.status = Failed.
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let link = Link::new(destination, tx);

    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let (resource_hash, _) =
        manager.start_send(&link, b"fail me".to_vec(), None).expect("start sender");
    manager.confirm_outbound_dispatch(resource_hash, true);

    assert!(manager.outgoing.contains_key(&resource_hash));
    assert!(manager.drain_events().is_empty());

    manager.outgoing.get_mut(&resource_hash).unwrap().status = ResourceStatus::Failed;

    let packets = manager.poll_outgoing(Instant::now());
    assert!(packets.is_empty());
    assert!(!manager.outgoing.contains_key(&resource_hash));

    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, resource_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

#[test]
fn resource_manager_link_close_allows_later_resource_on_new_link() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut first_link = Link::new(destination, tx.clone());
    first_link.request();

    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    // Random, not a repeated byte — see `resource_manager_times_out_
    // transferring_sender_after_retry_budget`'s identical comment: this
    // test depends on the payload splitting into 2 parts, which
    // auto-compression would otherwise collapse into 1.
    let mut first_payload = vec![0u8; PACKET_MDU + 24];
    OsRng.fill_bytes(&mut first_payload);
    let (first_hash, _) = manager.start_send(&first_link, first_payload, None).expect("first send");
    manager.confirm_outbound_dispatch(first_hash, true);

    let adv = ResourceAdvertisement {
        transfer_size: 1,
        data_size: 1,
        parts: 1,
        hash: Hash::new_from_slice(&[0x44; 32]),
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: Hash::new_from_slice(&[0x44; 32]),
        segment_index: 1,
        total_segments: 1,
        request_id: None,
        flags: 0,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    let incoming_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &adv.pack().expect("advertisement"),
        *first_link.id(),
    );
    let _ = manager.handle_packet(&incoming_packet, &mut first_link);
    manager.remove_link_state(*first_link.id());

    let mut second_link = Link::new(destination, tx);
    second_link.request();
    let mut second_payload = vec![0u8; PACKET_MDU + 24];
    OsRng.fill_bytes(&mut second_payload);
    let (second_hash, _) = manager.start_send(&second_link, second_payload, None).expect("second send");
    manager.confirm_outbound_dispatch(second_hash, true);

    assert!(!manager.outgoing.contains_key(&first_hash));
    assert!(manager.outgoing.contains_key(&second_hash));
    assert!(manager.incoming.is_empty());

    let first_map_hash = manager
        .outgoing
        .get(&second_hash)
        .expect("second outgoing sender")
        .map_hashes[0];
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: second_hash,
        requested_hashes: vec![first_map_hash],
    };
    let request_packet =
        resource_packet(PacketContext::ResourceRequest, &request.encode(), *second_link.id());
    let responses = manager.handle_packet(&request_packet, &mut second_link);

    assert_eq!(responses.len(), 1);
    assert_eq!(
        manager.outgoing.get(&second_hash).expect("second sender").status,
        ResourceStatus::Transferring
    );
}

/// A local cancel of an inbound transfer drops its receiver and reports it
/// `InboundFailed("cancelled")` under the original hash, once; a hash with
/// no receiver is a no-op.
#[test]
fn resource_manager_cancel_incoming_drops_the_receiver_and_reports_it_once() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    link.request();
    let mut manager = ResourceManager::new();

    let segment_hash = Hash::new_from_slice(&[0x45; 32]);
    let unknown_hash = Hash::new_from_slice(&[0x46; 32]);
    let adv = ResourceAdvertisement {
        transfer_size: 1,
        data_size: 1,
        parts: 1,
        hash: segment_hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: segment_hash,
        segment_index: 1,
        total_segments: 1,
        request_id: None,
        flags: 0,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    let incoming_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &adv.pack().expect("advertisement"),
        *link.id(),
    );
    let _ = manager.handle_packet(&incoming_packet, &mut link);
    manager.drain_events();
    assert!(manager.incoming.contains_key(&segment_hash));

    assert_eq!(manager.cancel_incoming(unknown_hash), None, "no receiver under that hash");
    assert_eq!(manager.cancel_incoming(segment_hash), Some(segment_hash));
    assert!(manager.incoming.is_empty());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, segment_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(
        &events[0].kind,
        ResourceEventKind::InboundFailed(failure) if failure.reason == "cancelled"
    ));
    assert_eq!(manager.cancel_incoming(segment_hash), None, "already gone");
    assert!(manager.drain_events().is_empty());
}

/// An advertisement the manager drops leaves no receiver and no event behind;
/// only an admitted one has an active receiver under its segment hash, and
/// only for the request id and link it was built for.
#[test]
fn resource_manager_has_an_active_receiver_only_for_an_admitted_advertisement() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    link.request();
    let mut manager = ResourceManager::new();

    let dropped_hash = Hash::new_from_slice(&[0x47; 32]);
    let admitted_hash = Hash::new_from_slice(&[0x48; 32]);
    let request_id = [0x51u8; ADDRESS_HASH_SIZE];
    let other_request_id = [0x52u8; ADDRESS_HASH_SIZE];
    let other_link_id = AddressHash::new_from_slice(&[0x53; ADDRESS_HASH_SIZE]);
    // `ResourceReceiver::new` copies `request_id` whatever the flags say; the
    // response flag is set because that is the advertisement a response
    // Resource carries (`RNS/Resource.py:1318`).
    let advertisement = |hash: Hash, parts: u32| ResourceAdvertisement {
        transfer_size: 1,
        data_size: 1,
        parts,
        hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash: hash,
        segment_index: 1,
        total_segments: 1,
        request_id: Some(ByteBuf::from(request_id.to_vec())),
        flags: FLAG_RESPONSE,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    // `ResourceReceiver::new` refuses a part count of zero.
    let dropped = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &advertisement(dropped_hash, 0).pack().expect("advertisement"),
        *link.id(),
    );
    let _ = manager.handle_packet(&dropped, &mut link);
    assert!(!manager.has_active_receiver(&dropped_hash, link.id(), &request_id));
    assert!(manager.drain_events().is_empty(), "a dropped advertisement reports nothing");

    let admitted = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &advertisement(admitted_hash, 1).pack().expect("advertisement"),
        *link.id(),
    );
    let _ = manager.handle_packet(&admitted, &mut link);
    assert!(manager.has_active_receiver(&admitted_hash, link.id(), &request_id));
    assert!(
        !manager.has_active_receiver(&admitted_hash, link.id(), &other_request_id),
        "the segment is active for the request it was advertised under only"
    );
    assert!(
        !manager.has_active_receiver(&admitted_hash, &other_link_id, &request_id),
        "and on the link it was advertised on only"
    );
}

/// Between one segment's completion and the next segment's advertisement a
/// split response is only a partial assembly. Cancelling the response for a
/// failed receipt drops that assembly when the hashes came from its link,
/// and leaves it alone for any other link.
#[test]
fn resource_manager_cancel_incoming_response_drops_only_its_links_assembly() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    link.request();
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);

    let first_data = b"first-segment";
    let second_data = b"second-segment";
    let total_data_size = (first_data.len() + second_data.len()) as u64;
    let (first_adv, first_part) = split_test_segment(first_data, None, 1, 2, total_data_size);
    let original_hash = first_adv.hash;
    let first_adv_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &first_adv.pack().expect("pack first advertisement"),
        *link.id(),
    );
    assert_eq!(manager.handle_packet(&first_adv_packet, &mut link).len(), 1);
    let first_part_packet = resource_packet(PacketContext::Resource, &first_part, *link.id());
    assert_eq!(manager.handle_packet(&first_part_packet, &mut link).len(), 1);
    manager.drain_events();
    assert!(manager.incoming_segments.contains_key(&original_hash));
    assert!(manager.incoming.is_empty(), "the first segment is complete, nothing is in flight");

    let other_link_id = AddressHash::new_from_slice(&[0x53; ADDRESS_HASH_SIZE]);
    assert!(!manager.cancel_incoming_response(original_hash, original_hash, &other_link_id));
    assert!(manager.incoming_segments.contains_key(&original_hash));
    assert!(manager.drain_events().is_empty(), "another link's cancel reports nothing");

    assert!(
        !manager.cancel_incoming_response(original_hash, original_hash, link.id()),
        "no receiver was in flight, so the peer is owed no receiver cancel"
    );
    assert!(!manager.incoming_segments.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(
        &events[0].kind,
        ResourceEventKind::InboundFailed(failure) if failure.reason == "cancelled"
    ));
}

/// While a response segment's receiver is in flight, cancelling the response
/// for a failed receipt drops that receiver and reports the transfer
/// `InboundFailed("cancelled")` once, and says so, since the peer is then owed
/// a receiver cancel; another link's cancel under the same hashes is a no-op.
#[test]
fn resource_manager_cancel_incoming_response_drops_its_links_live_receiver() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    link.request();
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);

    let segment_hash = Hash::new_from_slice(&[0x49; 32]);
    let original_hash = segment_hash;
    let adv = ResourceAdvertisement {
        transfer_size: 1,
        data_size: 1,
        parts: 1,
        hash: segment_hash,
        random_hash: [0u8; RANDOM_HASH_SIZE],
        original_hash,
        segment_index: 1,
        total_segments: 1,
        request_id: Some(ByteBuf::from([0x51u8; ADDRESS_HASH_SIZE].to_vec())),
        flags: FLAG_RESPONSE,
        hashmap: vec![0u8; MAPHASH_LEN],
    };
    let adv_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &adv.pack().expect("pack advertisement"),
        *link.id(),
    );
    let _ = manager.handle_packet(&adv_packet, &mut link);
    manager.drain_events();
    assert!(manager.incoming.contains_key(&segment_hash), "the receiver is in flight");

    let other_link_id = AddressHash::new_from_slice(&[0x53; ADDRESS_HASH_SIZE]);
    assert!(!manager.cancel_incoming_response(segment_hash, original_hash, &other_link_id));
    assert!(manager.incoming.contains_key(&segment_hash));
    assert!(manager.drain_events().is_empty(), "another link's cancel reports nothing");

    assert!(
        manager.cancel_incoming_response(segment_hash, original_hash, link.id()),
        "a receiver was in flight, so the peer is owed a receiver cancel"
    );
    assert!(!manager.incoming.contains_key(&segment_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(
        &events[0].kind,
        ResourceEventKind::InboundFailed(failure) if failure.reason == "cancelled"
    ));
    assert!(!manager.cancel_incoming_response(segment_hash, original_hash, link.id()));
    assert!(manager.drain_events().is_empty(), "already gone");
}
