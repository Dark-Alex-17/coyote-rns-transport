/// Undoes what `resource/utils.rs::fill_link_packet_data` did, and only when it
/// did it: fragments carry the sender's own ciphertext and proofs carry a
/// signature, so neither gets a second link-layer encryption. Decrypting them
/// anyway fails as `IncorrectSignature`.
fn decrypt_link_packet(link: &Link, packet: &Packet) -> Packet {
    let encrypted = packet.context != PacketContext::Resource
        && !(packet.header.packet_type == PacketType::Proof
            && packet.context == PacketContext::ResourceProof);
    if !encrypted {
        return packet.clone();
    }
    let mut plain = packet.clone();
    let mut buffer = PacketDataBuffer::new();
    let plain_len = {
        let text = link
            .decrypt(packet.data.as_slice(), buffer.accuire_buf_max())
            .expect("peer link should decrypt");
        text.len()
    };
    buffer.resize(plain_len);
    plain.data = buffer;
    plain
}

/// A split send must not build every segment before it can advertise the first
/// one. `start_send` runs with the transport handler mutex held, so the eager
/// loop this replaced put one bz2 + AES + SHA-256 pass over the whole payload,
/// and every fragment it would ever need, in front of the first advertisement —
/// a full second of global lock on a 46 MB send, and ~100k live `Vec<u8>`
/// fragments retained until the transfer finished.
#[test]
fn split_send_builds_only_the_first_segment_up_front() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let data = vec![0x5a; (MAX_EFFICIENT_SIZE * 3) + 17];

    let (original_hash, first_packet) =
        manager.start_send(&link, data, None).expect("start split resource");
    let first = decrypt_advertisement(&link, &first_packet);
    assert_eq!(first.total_segments, 4);

    // The whole point: exactly one sender exists, not yet advertised, and the
    // other three segments are still just an offset into the caller's payload.
    assert_eq!(manager.outgoing.len(), 1);
    assert_eq!(
        manager.outgoing.get(&first.hash).expect("unconfirmed sender").status,
        ResourceStatus::None
    );
    let pending = manager
        .outgoing_segment_chains
        .get(&original_hash)
        .expect("chain for the unbuilt tail");
    assert_eq!(pending.next_segment_index, Some(2));
    assert_eq!(pending.total_segments, 4);

    manager.confirm_outbound_dispatch(original_hash, true);

    // Each proof builds exactly one more segment, and never more than one.
    let mut advertised = first;
    for expected_index in 2..=4u32 {
        let expected_proof =
            manager.outgoing.get(&advertised.hash).expect("advertised sender").expected_proof;
        let proof = ResourceProof { resource_hash: advertised.hash, proof: expected_proof };
        let packets = manager.handle_packet(
            &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
            &mut link,
        );
        assert_eq!(packets.len(), 1, "one advertisement per proof");
        advertised = decrypt_advertisement(&link, &packets[0]);
        assert_eq!(advertised.segment_index, expected_index);
        assert_eq!(advertised.original_hash, original_hash);
        assert_eq!(
            manager.outgoing.len(),
            1,
            "only the in-flight segment is held, never the ones after it"
        );
    }
}

/// Building segments on demand must produce exactly the bytes the eager loop
/// did. This drives a real multi-segment transfer through a sender and a
/// receiver manager and compares the reassembled payload to the input, so a
/// wrong offset or a dropped tail byte cannot pass.
#[test]
fn lazily_built_segments_reassemble_byte_for_byte() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    // A genuine pair, so each side really can read the other's packets rather
    // than the test quietly measuring one link talking to itself.
    let mut sender_link = Link::new(destination, tx.clone());
    let request = sender_link.request();
    let mut receiver_link =
        Link::new_from_request(&request, signer.sign_key().clone(), destination, tx)
            .expect("link request should parse");
    assert!(matches!(
        sender_link.handle_packet(&receiver_link.prove(), AddressHash::new_from_rand(OsRng)),
        LinkHandleResult::Activated
    ));
    let mut sender = ResourceManager::new_with_config(Duration::from_secs(30), 8);
    let mut receiver = ResourceManager::new_with_config(Duration::from_secs(30), 8);

    // Deliberately not a round multiple of the segment size, and not uniform:
    // a payload of one repeated byte would reassemble "correctly" even if two
    // segments were swapped.
    let payload: Vec<u8> =
        (0..(MAX_EFFICIENT_SIZE * 2) + 4242).map(|index| (index % 251) as u8).collect();

    let (original_hash, advertisement) =
        sender.start_send(&sender_link, payload.clone(), None).expect("start split resource");
    sender.confirm_outbound_dispatch(original_hash, true);

    let mut to_receiver = vec![advertisement];
    let mut delivered: Option<Vec<u8>> = None;
    // Bounded so a stall fails the test instead of hanging it.
    for _ in 0..100_000 {
        if to_receiver.is_empty() {
            break;
        }
        let mut to_sender = Vec::new();
        for packet in std::mem::take(&mut to_receiver) {
            let plain = decrypt_link_packet(&receiver_link, &packet);
            to_sender.extend(receiver.handle_packet(&plain, &mut receiver_link));
        }
        for event in receiver.drain_events() {
            if let ResourceEventKind::Complete(complete) = event.kind {
                delivered = Some(complete.data);
            }
        }
        // Deliberately does not stop at delivery: the sender has not seen the
        // final proof yet, and the tail is only released when it does.
        for packet in to_sender {
            let plain = decrypt_link_packet(&sender_link, &packet);
            to_receiver.extend(sender.handle_packet(&plain, &mut sender_link));
        }
        sender.drain_events();
    }

    assert_eq!(delivered.as_deref(), Some(payload.as_slice()));
    // The tail is released once the last segment is built and proved — nothing
    // is left holding a copy of the payload after the transfer.
    assert!(!sender.outgoing_segment_chains.contains_key(&original_hash));
    assert!(sender.has_no_outbound_state());
}

/// A segment that cannot be built has no caller left to return an error to —
/// `handle_proof_into` is several frames deep in the packet path. Reporting it
/// as `OutboundFailed` is what keeps a dead transfer from looking live to both
/// ends, the same trap #557 closed for inbound assemblies.
#[test]
fn a_segment_that_cannot_be_built_reports_outbound_failure() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let data = vec![0x5a; MAX_EFFICIENT_SIZE + 257];

    let (original_hash, _) = manager.start_send(&link, data, None).expect("start split resource");
    manager.confirm_outbound_dispatch(original_hash, true);

    // An MTU too small to carry a resource fragment makes the *next* build
    // fail, without disturbing the segment already advertised.
    manager
        .outgoing_segment_chains
        .get_mut(&original_hash)
        .expect("pending tail")
        .interface_mtu = 1;

    let expected_proof =
        manager.outgoing.get(&original_hash).expect("first sender").expected_proof;
    let proof = ResourceProof { resource_hash: original_hash, proof: expected_proof };
    let packets = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );

    assert!(packets.is_empty(), "a failed build must not advertise anything");
    assert!(!manager.outgoing_segment_chains.contains_key(&original_hash));
    let events = manager.drain_events();
    assert!(
        events.iter().any(|event| event.hash == original_hash
            && matches!(event.kind, ResourceEventKind::OutboundFailed)),
        "a build failure must be reported, not dropped: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, ResourceEventKind::OutboundComplete)),
        "a failed transfer must not also report completion"
    );
}

/// Closing a link has to drop the unbuilt tail with it. The previous shape
/// keyed this off the front sender's `link_id`, which also meant a chain
/// drained to empty matched nothing and was retained for the process lifetime.
#[test]
fn removing_a_link_drops_its_unbuilt_segments() {
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
    let data = vec![0x5a; MAX_EFFICIENT_SIZE + 257];

    let (original_hash, _) = manager.start_send(&link, data, None).expect("start split resource");
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));

    manager.remove_link_state(*link.id());
    assert!(manager.outgoing_segment_chains.is_empty());
    assert!(manager.has_no_outbound_state());
    let events = manager.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].hash, original_hash);
    assert!(matches!(events[0].kind, ResourceEventKind::OutboundFailed));
}

/// A peer cancelling mid-transfer has to drop the unbuilt tail too.
///
/// The trap is that a cancel names the **segment** in flight, not the split
/// resource: from segment two onwards its hash is nothing like `original_hash`,
/// which is what the tail is keyed by. Removing only the sender therefore looks
/// like a clean cancel while leaving the entire remaining payload alive until
/// the link closes — and a link that is being reused rather than torn down may
/// not close for a long time.
#[test]
fn a_remote_cancel_drops_the_unbuilt_tail() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let data = vec![0x5a; (MAX_EFFICIENT_SIZE * 3) + 17];

    let (original_hash, _) =
        manager.start_send(&link, data, None).expect("start split resource");
    manager.confirm_outbound_dispatch(original_hash, true);

    // Advance to segment two, so the cancel names a hash that is *not* the key
    // the tail is stored under. Cancelling segment one would pass even with the
    // bug, since there `original_hash` is the sender's own hash.
    let expected_proof =
        manager.outgoing.get(&original_hash).expect("first sender").expected_proof;
    let proof = ResourceProof { resource_hash: original_hash, proof: expected_proof };
    let packets = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof.encode(), *link.id()),
        &mut link,
    );
    let second = decrypt_advertisement(&link, &packets[0]);
    assert_eq!(second.segment_index, 2);
    assert_ne!(second.hash, original_hash, "the test is only meaningful if these differ");
    assert_eq!(
        manager.outgoing_segment_chains.get(&original_hash).expect("tail").next_segment_index,
        Some(3)
    );

    // We are the initiator, so a cancel from the peer is a receiver cancel.
    manager.handle_packet(
        &resource_packet(
            PacketContext::ResourceReceiverCancel,
            second.hash.as_slice(),
            *link.id(),
        ),
        &mut link,
    );

    assert!(
        !manager.outgoing_segment_chains.contains_key(&original_hash),
        "a cancelled split send must not keep holding the rest of the payload"
    );
    assert!(manager.has_no_outbound_state());
    assert!(manager
        .drain_events()
        .iter()
        .any(|event| matches!(event.kind, ResourceEventKind::OutboundRejected)));
}

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
    let data = vec![0x5a; MAX_EFFICIENT_SIZE + 257];
    let (original_hash, first_packet) =
        manager.start_send(&link, data, None).expect("start split resource");
    let first = decrypt_advertisement(&link, &first_packet);
    (link, manager, original_hash, first)
}

/// The advertisement is written to the wire before the caller can confirm
/// it, so a fast peer's first part request may land in that window. It has
/// to be served from the sender exactly as a later one would be, while the
/// retry poll leaves the still-unconfirmed sender alone.
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
        ResourceStatus::Advertised
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

/// The same cancel, arriving before the first advertisement has been confirmed
/// as dispatched: the sender sits unconfirmed in `outgoing`, and the tail
/// chain still has to go with it.
#[test]
fn a_remote_cancel_before_dispatch_drops_the_unbuilt_tail() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "resource"),
    };
    let (tx, _) = tokio::sync::broadcast::channel(1);
    let mut link = Link::new(destination, tx);
    let mut manager = ResourceManager::new_with_config(Duration::from_secs(1), 2);
    let data = vec![0x5a; MAX_EFFICIENT_SIZE + 257];

    let (original_hash, _) =
        manager.start_send(&link, data, None).expect("start split resource");
    assert!(manager.outgoing_segment_chains.contains_key(&original_hash));

    manager.handle_packet(
        &resource_packet(
            PacketContext::ResourceReceiverCancel,
            original_hash.as_slice(),
            *link.id(),
        ),
        &mut link,
    );

    assert!(manager.outgoing_segment_chains.is_empty());
    assert!(manager.has_no_outbound_state());
    assert!(manager
        .drain_events()
        .iter()
        .any(|event| matches!(event.kind, ResourceEventKind::OutboundRejected)));
}
