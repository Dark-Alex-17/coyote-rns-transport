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
