/// Requests the part at `index` of `advert` from the manager, the way a peer
/// that still holds the first hashmap window would.
fn request_part(
    manager: &mut ResourceManager,
    link: &mut Link,
    advert: &ResourceAdvertisement,
    index: usize,
) -> Vec<Packet> {
    let mut map_hash = [0u8; MAPHASH_LEN];
    map_hash.copy_from_slice(&advert.hashmap[index * MAPHASH_LEN..(index + 1) * MAPHASH_LEN]);
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: advert.hash,
        requested_hashes: vec![map_hash],
    };
    manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &request.encode(), *link.id()),
        link,
    )
}

/// Once the confirm has carried an early-served part into `Transferring`,
/// a second confirm finds a sender that is no longer unconfirmed and must
/// leave it exactly where it is: no re-advertisement, no event, no status
/// change, and the unbuilt tail still parked.
#[test]
fn a_repeated_dispatch_confirm_leaves_a_transferring_sender_alone() {
    let (mut link, mut manager, original_hash, first) = unconfirmed_split_send();
    assert_eq!(request_part(&mut manager, &mut link, &first, 0).len(), 1);

    manager.confirm_outbound_dispatch(first.hash, true);
    manager.confirm_outbound_dispatch(first.hash, true);

    assert_eq!(manager.outgoing.len(), 1);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("transferring sender").status,
        ResourceStatus::Transferring
    );
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));
    assert!(manager.drain_events().is_empty());
    assert!(
        manager.poll_outgoing(Instant::now() + Duration::from_secs(1)).is_empty(),
        "a repeated confirm must not re-advertise a transferring sender"
    );
    assert_eq!(request_part(&mut manager, &mut link, &first, 1).len(), 1);
}

/// The same for a sender the late confirm moved to `AwaitingProof`: a second
/// confirm must not hand it a fresh advertisement budget, and the proof that
/// then lands still completes it.
#[test]
fn a_repeated_dispatch_confirm_leaves_an_awaiting_proof_sender_alone() {
    let (mut link, mut manager, hash) = unconfirmed_fully_served_send();

    manager.confirm_outbound_dispatch(hash, true);
    manager.confirm_outbound_dispatch(hash, true);

    assert_eq!(
        manager.outgoing.get(&hash).expect("confirmed sender").status,
        ResourceStatus::AwaitingProof
    );
    assert!(manager.poll_outgoing(Instant::now() + Duration::from_secs(1)).is_empty());
    assert!(manager.drain_events().is_empty());

    let expected_proof = manager.outgoing.get(&hash).expect("sender").expected_proof;
    let proof = ResourceProof { resource_hash: hash, proof: expected_proof };
    manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundComplete));
}

/// A failed advertisement write settles the sender no matter what the peer
/// managed to pull out of it in the meantime: the sender and its tail go,
/// the failure is reported once, and a request that lands afterwards finds
/// nothing to serve.
#[test]
fn a_failed_dispatch_confirm_after_served_parts_still_drops_the_sender() {
    let (mut link, mut manager, original_hash, first) = unconfirmed_split_send();
    assert_eq!(request_part(&mut manager, &mut link, &first, 0).len(), 1);

    manager.confirm_outbound_dispatch(first.hash, false);

    assert!(manager.has_no_outbound_state());
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, first.hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));

    assert!(
        request_part(&mut manager, &mut link, &first, 1).is_empty(),
        "a dropped sender serves nothing"
    );
    assert!(manager.has_no_outbound_state());
    assert!(manager.drain_events().is_empty());
}

/// A fully served single-part send whose only part went out before the
/// confirm, and whose proof ALSO lands before the confirm. The proof must
/// count: the transfer is complete, reported once, and the confirm that
/// arrives afterwards has nothing left to settle. Dropping the proof instead
/// would park a finished transfer in `AwaitingProof` until the proof budget
/// fails it.
#[test]
fn a_proof_before_the_dispatch_confirm_completes_the_sender() {
    let (mut link, mut manager, hash) = unconfirmed_fully_served_send();

    let expected_proof = manager.outgoing.get(&hash).expect("unconfirmed sender").expected_proof;
    let proof = ResourceProof { resource_hash: hash, proof: expected_proof };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    assert!(responses.is_empty());
    assert!(manager.has_no_outbound_state(), "the proof completes the unconfirmed sender");
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundComplete));

    manager.confirm_outbound_dispatch(hash, true);
    assert!(manager.has_no_outbound_state());
    assert!(manager.drain_events().is_empty(), "the late confirm has nothing to settle");
    assert!(manager.poll_outgoing(Instant::now() + Duration::from_secs(600)).is_empty());
}

/// A split send whose first segment served a part before the confirm, was
/// confirmed into `Transferring`, and is then proved: the chain must hand
/// off to the second segment exactly as it does for a sender that was never
/// unconfirmed, and the second segment's advertisement must be live in the
/// retry pump rather than stranded waiting for a confirm nobody will give.
#[test]
fn a_segment_served_before_the_confirm_still_hands_off_to_the_next_segment() {
    let (mut link, mut manager, original_hash, first) = unconfirmed_split_send();
    assert_eq!(first.total_segments, 2);
    assert_eq!(request_part(&mut manager, &mut link, &first, 0).len(), 1);
    manager.confirm_outbound_dispatch(first.hash, true);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("confirmed sender").status,
        ResourceStatus::Transferring
    );

    let expected_proof = manager.outgoing.get(&first.hash).expect("sender").expected_proof;
    let proof = ResourceProof { resource_hash: first.hash, proof: expected_proof };
    let packets = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    assert_eq!(packets.len(), 1, "the proof builds the next segment's advertisement");
    let second = decrypt_advertisement(&link, &packets[0]);
    assert_eq!(second.segment_index, 2);
    assert_eq!(second.original_hash, original_hash);
    assert_ne!(second.hash, first.hash);
    assert!(!manager.outgoing.contains_key(&first.hash));
    assert_eq!(manager.outgoing.len(), 1);

    let second_status = manager.outgoing.get(&second.hash).expect("second segment").status;
    assert_ne!(
        second_status,
        ResourceStatus::None,
        "a segment built from a proof has no confirm step, it must not wait for one"
    );
    let now = Instant::now() + Duration::from_secs(2);
    let retries = manager.poll_outgoing(now);
    assert_eq!(retries.len(), 1, "the second segment's advertisement is retried");
    assert_eq!(retries[0].1.context, PacketContext::ResourceAdvrtisement);
    assert_eq!(decrypt_advertisement(&link, &retries[0].1).hash, second.hash);
    assert_eq!(request_part(&mut manager, &mut link, &second, 0).len(), 1);
    assert!(manager.drain_events().is_empty());

    let expected_proof = manager.outgoing.get(&second.hash).expect("sender").expected_proof;
    let proof = ResourceProof { resource_hash: second.hash, proof: expected_proof };
    let packets = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    assert!(packets.is_empty(), "the last segment has nothing to hand off to");
    assert!(manager.has_no_outbound_state());
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundComplete));
}

/// A remote cancel after a part was served but before the confirm: the
/// sender goes with its tail and the confirm that lands afterwards is a
/// no-op, the same as for a sender that never served anything. A receiver
/// cancel is reported as `OutboundRejected`, as it is for a confirmed sender.
#[test]
fn a_remote_cancel_after_served_parts_before_the_confirm_drops_the_sender() {
    let (mut link, mut manager, original_hash, first) = unconfirmed_split_send();
    assert_eq!(request_part(&mut manager, &mut link, &first, 0).len(), 1);

    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceReceiverCancel, first.hash.as_slice(), *link.id()),
        &mut link,
    );
    assert!(responses.is_empty());
    assert!(manager.has_no_outbound_state());
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundRejected));

    manager.confirm_outbound_dispatch(first.hash, true);
    assert!(manager.has_no_outbound_state());
    assert!(manager.drain_events().is_empty());
    assert!(manager.poll_outgoing(Instant::now() + Duration::from_secs(600)).is_empty());
}
