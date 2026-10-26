fn second_test_link() -> Link {
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
    link
}

/// An initiator cancel arriving on one link names a receiver that belongs to
/// another link: the receiver stays in flight and nothing is reported.
#[test]
fn resource_manager_ignores_initiator_cancel_from_another_link() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
    assert_ne!(link_a.id(), link_b.id());

    let segment_hash = Hash::new_from_slice(&[0x4a; 32]);
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
    let adv_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &adv.pack().expect("pack advertisement"),
        *link_a.id(),
    );
    let _ = manager.handle_packet(&adv_packet, &mut link_a);
    manager.drain_events();
    assert!(manager.incoming.contains_key(&segment_hash), "the receiver is in flight");

    let cancel = resource_packet(
        PacketContext::ResourceInitiatorCancel,
        segment_hash.as_slice(),
        *link_b.id(),
    );
    let responses = manager.handle_packet(&cancel, &mut link_b);
    assert!(responses.is_empty());
    assert!(
        manager.incoming.contains_key(&segment_hash),
        "another link's cancel leaves the receiver"
    );
    assert!(manager.drain_events().is_empty(), "another link's cancel reports nothing");
}

/// A receiver cancel arriving on one link names a sender that belongs to
/// another link: the sender stays and nothing is reported.
#[test]
fn resource_manager_ignores_receiver_cancel_from_another_link() {
    let (mut manager, link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
    let (resource_hash, _) =
        manager.start_send(&link_a, b"stays".to_vec(), None).expect("start sender");
    manager.confirm_outbound_dispatch(resource_hash, true);

    let cancel = resource_packet(
        PacketContext::ResourceReceiverCancel,
        resource_hash.as_slice(),
        *link_b.id(),
    );
    let responses = manager.handle_packet(&cancel, &mut link_b);
    assert!(responses.is_empty());
    assert!(
        manager.outgoing.contains_key(&resource_hash),
        "another link's cancel leaves the sender"
    );
    assert!(manager.drain_events().is_empty(), "another link's cancel reports nothing");
}

/// A one-part, unsplit resource with `data` as its payload, together with the one part
/// that completes it.
fn one_part_resource(
    data: &[u8],
    random_hash: [u8; RANDOM_HASH_SIZE],
) -> (ResourceAdvertisement, Vec<u8>) {
    let mut part = Vec::with_capacity(RANDOM_HASH_SIZE + data.len());
    part.extend_from_slice(&random_hash);
    part.extend_from_slice(data);
    let resource_hash = Hash::new_from_slice(&[data, random_hash.as_slice()].concat());
    let mut hashmap = Vec::with_capacity(MAPHASH_LEN);
    hashmap.extend_from_slice(&map_hash(&part, &random_hash));
    (
        ResourceAdvertisement {
            transfer_size: part.len() as u64,
            data_size: data.len() as u64,
            parts: 1,
            hash: resource_hash,
            random_hash,
            original_hash: resource_hash,
            segment_index: 1,
            total_segments: 1,
            request_id: None,
            flags: 0,
            hashmap,
        },
        part,
    )
}

fn has_complete_for(events: &[ResourceEvent], hash: &Hash) -> bool {
    events
        .iter()
        .any(|event| event.hash == *hash && matches!(event.kind, ResourceEventKind::Complete(_)))
}

/// A part arriving on one link that matches a receiver advertised on another link is not
/// fed to that receiver: the assembly stays incomplete and the same part on its own link
/// still completes it.
#[test]
fn resource_manager_ignores_a_part_from_another_link() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
    let (adv, part) = one_part_resource(b"belongs to link a", [0x11, 0x22, 0x33, 0x44]);
    let adv_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &adv.pack().expect("pack advertisement"),
        *link_a.id(),
    );
    assert_eq!(manager.handle_packet(&adv_packet, &mut link_a).len(), 1, "parts requested");
    manager.drain_events();

    let responses = manager
        .handle_packet(&resource_packet(PacketContext::Resource, &part, *link_b.id()), &mut link_b);
    let events = manager.drain_events();
    assert!(responses.is_empty(), "another link's part earns no proof or request: {responses:?}");
    assert!(!has_complete_for(&events, &adv.hash), "another link's part completes nothing");
    assert_eq!(
        manager.incoming[&adv.hash].progress().received_parts,
        0,
        "another link's part is not counted against the receiver"
    );

    let responses = manager
        .handle_packet(&resource_packet(PacketContext::Resource, &part, *link_a.id()), &mut link_a);
    let events = manager.drain_events();
    assert_eq!(responses.len(), 1, "the part on its own link is proven: {events:?}");
    assert!(has_complete_for(&events, &adv.hash), "the part on its own link completes");
}

/// The next segment of a split resource advertised on one link is not admitted when it
/// arrives on another link naming the same original hash: that link has no assembly in
/// progress, so its first expected segment is 1 and a segment 2 is dropped unanswered.
#[test]
fn resource_manager_ignores_a_split_segment_from_another_link() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
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
    assert_eq!(manager.handle_packet(&first_packet, &mut link_a).len(), 1);
    assert_eq!(
        manager
            .handle_packet(
                &resource_packet(PacketContext::Resource, &first_part, *link_a.id()),
                &mut link_a
            )
            .len(),
        1,
        "the first segment is proven on link a"
    );
    manager.drain_events();

    let (second_adv, _) = split_test_segment(second_data, Some(original_hash), 2, 2, total);
    let second_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &second_adv.pack().expect("second advertisement"),
        *link_b.id(),
    );
    let responses = manager.handle_packet(&second_packet, &mut link_b);
    assert!(responses.is_empty(), "another link's segment 2 is not requested: {responses:?}");
    assert!(
        !manager.incoming.contains_key(&second_adv.hash),
        "another link's segment 2 does not join link a's assembly"
    );
    assert!(manager.drain_events().is_empty(), "another link's segment 2 reports nothing");
}

/// An initiator cancel on one link tears down only that link's transfer. Link a has
/// segment 1 assembled and segment 2 in flight; link b is admitted a one-segment
/// resource whose hash is link a's original hash, then cancels it. Link b's receiver
/// goes and is reported under link b; link a's assembly and in-flight segment stay.
#[test]
fn resource_manager_keeps_a_split_assembly_through_another_links_initiator_cancel() {
    let (mut manager, mut link_a) = manager_with_test_link();
    let mut link_b = second_test_link();
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
    assert_eq!(manager.handle_packet(&first_packet, &mut link_a).len(), 1);
    assert_eq!(
        manager
            .handle_packet(
                &resource_packet(PacketContext::Resource, &first_part, *link_a.id()),
                &mut link_a
            )
            .len(),
        1,
        "the first segment is proven on link a"
    );
    let (second_adv, _) = split_test_segment(second_data, Some(original_hash), 2, 2, total);
    let second_packet = resource_packet(
        PacketContext::ResourceAdvrtisement,
        &second_adv.pack().expect("second advertisement"),
        *link_a.id(),
    );
    assert_eq!(manager.handle_packet(&second_packet, &mut link_a).len(), 1);
    manager.drain_events();
    assert!(manager.incoming_segments.contains_key(&original_hash), "segment 1 is assembled");
    assert!(manager.incoming.contains_key(&second_adv.hash), "segment 2 is in flight");

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
    assert_eq!(manager.handle_packet(&forged_packet, &mut link_b).len(), 1, "admitted on link b");
    manager.drain_events();

    let cancel = resource_packet(
        PacketContext::ResourceInitiatorCancel,
        original_hash.as_slice(),
        *link_b.id(),
    );
    assert!(manager.handle_packet(&cancel, &mut link_b).is_empty());
    assert!(!manager.incoming.contains_key(&original_hash), "link b's receiver is gone");
    assert!(manager.incoming_segments.contains_key(&original_hash), "link a's assembly stays");
    assert!(manager.incoming.contains_key(&second_adv.hash), "link a's segment 2 stays");
    let events = manager.drain_events();
    assert_eq!(events.len(), 1, "only link b's receiver is reported: {events:?}");
    assert_eq!(events[0].link_id, *link_b.id(), "the failure is link b's");
    assert!(matches!(events[0].kind, ResourceEventKind::InboundFailed(_)));
}
