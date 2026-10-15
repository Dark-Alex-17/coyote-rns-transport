/// A split send whose first advertisement has been built but whose dispatch
/// has not been confirmed yet: the sender sits in `outgoing` with status
/// `None`. The link has issued its request so it can encrypt a cancel frame.
fn unconfirmed_split_send() -> (Link, ResourceManager, Hash, ResourceAdvertisement) {
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
    // Random, not a repeated byte: a uniform run this size auto-compresses
    // to a single part, and the early-request test below needs the first
    // segment to have parts left over after one is served.
    let mut data = vec![0u8; MAX_EFFICIENT_SIZE + 257];
    OsRng.fill_bytes(&mut data);
    let (original_hash, first_packet) =
        manager.start_send(&link, data, None).expect("start split resource");
    let first = decrypt_advertisement(&link, &first_packet);
    (link, manager, original_hash, first)
}

/// The advertisement is written to the wire before the caller can confirm
/// it, so a fast peer's first part request may land in that window. It has
/// to be served from the sender exactly as a later one would be, while the
/// retry poll leaves the still-unconfirmed sender alone. When the confirm
/// then lands it must account for the part already served: the sender goes
/// straight to `Transferring`, so the retry poll does not re-advertise it
/// and later requests keep being served.
#[test]
fn a_part_request_before_the_dispatch_confirm_is_served() {
    let (mut link, mut manager, _, first) = unconfirmed_split_send();
    let mut first_map_hash = [0u8; MAPHASH_LEN];
    first_map_hash.copy_from_slice(&first.hashmap[..MAPHASH_LEN]);

    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: first.hash,
        requested_hashes: vec![first_map_hash],
    };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &request.encode(), *link.id()),
        &mut link,
    );
    assert_eq!(responses.len(), 1, "early request is served before the confirm");
    assert_eq!(responses[0].context, PacketContext::Resource);

    let retries = manager.poll_outgoing(Instant::now() + Duration::from_secs(600));
    assert!(retries.is_empty(), "nothing is retried for an unconfirmed sender");
    assert!(manager.outgoing.contains_key(&first.hash));
    assert!(!manager
        .drain_events()
        .iter()
        .any(|event| matches!(event.kind, ResourceEventKind::OutboundFailed)));

    manager.confirm_outbound_dispatch(first.hash, true);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("confirmed sender").status,
        ResourceStatus::Transferring
    );

    let retries = manager.poll_outgoing(Instant::now() + Duration::from_secs(1));
    assert!(retries.is_empty(), "a sender already serving parts is not re-advertised");
    assert!(manager.outgoing.contains_key(&first.hash));

    let mut second_map_hash = [0u8; MAPHASH_LEN];
    second_map_hash.copy_from_slice(&first.hashmap[MAPHASH_LEN..2 * MAPHASH_LEN]);
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: first.hash,
        requested_hashes: vec![second_map_hash],
    };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &request.encode(), *link.id()),
        &mut link,
    );
    assert_eq!(responses.len(), 1, "a later request is still served");
    assert_eq!(responses[0].context, PacketContext::Resource);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("transferring sender").status,
        ResourceStatus::Transferring
    );
}

#[test]
fn a_failed_dispatch_confirm_drops_the_sender_and_the_unbuilt_tail() {
    let (link, mut manager, original_hash, first) = unconfirmed_split_send();
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));

    manager.confirm_outbound_dispatch(first.hash, false);

    assert!(!manager.outgoing.contains_key(&first.hash));
    assert!(manager.outgoing_segment_chains.is_empty());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, first.hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

/// A local cancel racing the advertisement write finds the sender already
/// registered, so the peer gets an initiator cancel instead of a silent
/// disappearance.
#[test]
fn a_local_cancel_before_the_dispatch_confirm_sends_an_initiator_cancel() {
    let (link, mut manager, original_hash, first) = unconfirmed_split_send();

    let cancel_packet = manager
        .cancel_outgoing(original_hash, &link)
        .expect("cancel packet")
        .expect("unconfirmed sender still produces a cancel frame");

    assert!(manager.has_no_outbound_state());
    assert!(manager.outgoing_segment_chains.is_empty());
    assert_eq!(cancel_packet.destination, *link.id());
    assert_eq!(cancel_packet.context, PacketContext::ResourceInitiatorCancel);
    let mut decrypted = PacketDataBuffer::new();
    let plain_len = {
        let plain = link
            .decrypt(cancel_packet.data.as_slice(), decrypted.accuire_buf_max())
            .expect("decrypt cancel packet");
        plain.len()
    };
    decrypted.resize(plain_len);
    assert_eq!(decrypted.as_slice(), first.hash.as_slice());

    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundCancelled));
}

/// An early request can drive the still-unconfirmed sender to `Failed`. The
/// dispatch confirm must not paper over that by re-marking it advertised: the
/// terminal status stays put for the retry poll to report once.
#[test]
fn a_sender_failed_before_the_dispatch_confirm_stays_failed() {
    let (mut link, mut manager, original_hash, first) = unconfirmed_split_send();
    let mut first_map_hash = [0u8; MAPHASH_LEN];
    first_map_hash.copy_from_slice(&first.hashmap[..MAPHASH_LEN]);

    // Claiming the hashmap is exhausted at the first entry puts the next part
    // index at 1, which is never a segment boundary: a sequencing error.
    let request = ResourceRequest {
        hashmap_exhausted: true,
        last_map_hash: Some(first_map_hash),
        resource_hash: first.hash,
        requested_hashes: Vec::new(),
    };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &request.encode(), *link.id()),
        &mut link,
    );
    assert!(responses.is_empty(), "a sequencing error produces no parts");
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("unconfirmed sender").status,
        ResourceStatus::Failed
    );

    manager.confirm_outbound_dispatch(first.hash, true);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("confirmed sender").status,
        ResourceStatus::Failed,
        "the confirm must not revive a failed sender"
    );

    let retries = manager.poll_outgoing(Instant::now());
    assert!(retries.is_empty());
    assert!(!manager.outgoing.contains_key(&first.hash));
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

/// Link teardown while the advertisement write is still in flight: the
/// unconfirmed sender and its tail go with the link, and the confirm that
/// lands afterwards finds nothing to settle.
#[test]
fn a_link_teardown_before_the_dispatch_confirm_fails_the_sender_once() {
    let (link, mut manager, original_hash, first) = unconfirmed_split_send();

    manager.remove_link_state(*link.id());
    assert!(manager.has_no_outbound_state());
    assert!(manager.outgoing_segment_chains.is_empty());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));

    manager.confirm_outbound_dispatch(first.hash, true);
    assert!(manager.has_no_outbound_state());
    assert!(manager.drain_events().is_empty());
}

/// The caller confirms a dispatch once, but nothing stops a second confirm
/// from landing on the same hash. It must find the sender already advertised
/// and leave it alone, and the retry pump must then run the advertisement
/// budget exactly as it does for a sender that was never unconfirmed.
#[test]
fn a_repeated_dispatch_confirm_leaves_the_advertised_sender_alone() {
    let (link, mut manager, original_hash, first) = unconfirmed_split_send();

    manager.confirm_outbound_dispatch(first.hash, true);
    manager.confirm_outbound_dispatch(first.hash, true);

    assert_eq!(manager.outgoing.len(), 1, "one sender, not re-registered");
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("advertised sender").status,
        ResourceStatus::Advertised
    );
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));
    assert!(manager.drain_events().is_empty(), "a repeated confirm emits nothing");

    // retry_interval 1 s, retry_limit 2: two advertisement retries, then the
    // budget exhausts and the sender is reported failed once.
    let now = Instant::now() + Duration::from_secs(2);
    let first_retry = manager.poll_outgoing(now);
    assert_eq!(first_retry.len(), 1, "the confirmed sender is retried");
    assert_eq!(first_retry[0].0, *link.id());
    assert_eq!(first_retry[0].1.context, PacketContext::ResourceAdvrtisement);
    assert_eq!(first_retry[0].1.destination, *link.id());

    let second_retry = manager.poll_outgoing(now + Duration::from_secs(2));
    assert_eq!(second_retry.len(), 1);
    assert!(manager.drain_events().is_empty());

    let exhausted = manager.poll_outgoing(now + Duration::from_secs(4));
    assert!(exhausted.is_empty());
    assert!(!manager.outgoing.contains_key(&first.hash));
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

/// A failed confirm only ever settles the sender it names: an unknown hash
/// changes nothing, and a second failed confirm for a sender already dropped
/// does not report the failure twice.
#[test]
fn a_failed_dispatch_confirm_settles_only_the_sender_it_names() {
    let (link, mut manager, original_hash, first) = unconfirmed_split_send();

    let unknown = Hash::new_from_rand(OsRng);
    assert_ne!(unknown, first.hash);
    manager.confirm_outbound_dispatch(unknown, false);

    assert_eq!(manager.outgoing.len(), 1, "the real sender is untouched");
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("unconfirmed sender").status,
        ResourceStatus::None
    );
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));
    assert!(manager.drain_events().is_empty(), "an unknown hash emits nothing");

    manager.confirm_outbound_dispatch(first.hash, false);
    assert!(manager.has_no_outbound_state());
    assert!(manager.outgoing_segment_chains.is_empty());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, first.hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));

    manager.confirm_outbound_dispatch(first.hash, false);
    assert!(manager.has_no_outbound_state());
    assert!(
        manager.drain_events().is_empty(),
        "a sender already dropped is not failed a second time"
    );

    manager.confirm_outbound_dispatch(first.hash, true);
    assert!(manager.has_no_outbound_state());
    assert!(manager.drain_events().is_empty(), "a late confirm after the drop is a no-op");
}

/// A single-part send whose only part a fast peer requested before the
/// dispatch was confirmed. The sender is still `None` with every part
/// served and the proof outstanding.
fn unconfirmed_fully_served_send() -> (Link, ResourceManager, Hash) {
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
    let (hash, packet) = manager
        .start_send(&link, b"proof please".to_vec(), None)
        .expect("start single-part resource");
    let advert = decrypt_advertisement(&link, &packet);

    let mut map_hash = [0u8; MAPHASH_LEN];
    map_hash.copy_from_slice(&advert.hashmap[..MAPHASH_LEN]);
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: hash,
        requested_hashes: vec![map_hash],
    };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &request.encode(), *link.id()),
        &mut link,
    );
    assert_eq!(responses.len(), 1, "the only part is served before the confirm");
    assert_eq!(responses[0].context, PacketContext::Resource);
    assert_eq!(
        manager.outgoing.get(&hash).expect("unconfirmed sender").status,
        ResourceStatus::None
    );
    (link, manager, hash)
}

/// When every part was served before the confirm, the confirm must leave
/// the sender waiting for the proof rather than advertised: re-sending the
/// advertisement would have the peer start the transfer over.
#[test]
fn a_fully_served_sender_confirmed_late_awaits_the_proof() {
    let (mut link, mut manager, hash) = unconfirmed_fully_served_send();

    manager.confirm_outbound_dispatch(hash, true);
    assert_eq!(
        manager.outgoing.get(&hash).expect("confirmed sender").status,
        ResourceStatus::AwaitingProof
    );

    let retries = manager.poll_outgoing(Instant::now() + Duration::from_secs(1));
    assert!(retries.is_empty(), "the advertisement is not re-sent");
    assert!(manager.outgoing.contains_key(&hash));
    assert!(manager.drain_events().is_empty());

    let expected_proof = manager.outgoing.get(&hash).expect("sender").expected_proof;
    let proof = ResourceProof { resource_hash: hash, proof: expected_proof };
    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    assert!(responses.is_empty());
    assert!(!manager.outgoing.contains_key(&hash));
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundComplete));
}

/// The late confirm hands a fully served sender the same bounded proof
/// budget a post-confirm request would have: with retry_limit 2 that is two
/// retry intervals, after which the sender is dropped and reported failed
/// once.
#[test]
fn a_fully_served_sender_confirmed_late_times_out_on_the_proof_budget() {
    let (link, mut manager, hash) = unconfirmed_fully_served_send();

    manager.confirm_outbound_dispatch(hash, true);
    let now = Instant::now();

    assert!(manager.poll_outgoing(now + Duration::from_secs(1)).is_empty());
    assert!(manager.outgoing.contains_key(&hash));
    assert!(manager.poll_outgoing(now + Duration::from_secs(2)).is_empty());
    assert!(manager.outgoing.contains_key(&hash));
    assert!(manager.drain_events().is_empty());

    assert!(manager.poll_outgoing(now + Duration::from_secs(3)).is_empty());
    assert!(!manager.outgoing.contains_key(&hash));
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, hash);
    assert_eq!(events[0].link_id, *link.id());
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}
