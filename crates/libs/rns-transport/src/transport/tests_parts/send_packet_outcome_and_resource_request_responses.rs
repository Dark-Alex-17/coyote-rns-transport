#[tokio::test]
async fn send_packet_with_outcome_reports_missing_identity() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &identity, true);
    let transport = Transport::new(config);

    let packet = Packet { destination: AddressHash::new_from_rand(OsRng), ..Default::default() };
    let outcome = transport.send_packet_with_outcome(packet).await;

    assert_eq!(outcome, SendPacketOutcome::DroppedMissingDestinationIdentity);
}

#[tokio::test]
async fn send_packet_with_outcome_reports_no_route() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &identity, false);
    let transport = Transport::new(config);

    let packet = Packet {
        header: Header { packet_type: PacketType::Data, ..Default::default() },
        context: PacketContext::KeepAlive,
        data: PacketDataBuffer::new_from_slice(&[KEEP_ALIVE_REQUEST]),
        destination: AddressHash::new_from_rand(OsRng),
        ..Default::default()
    };
    let outcome = transport.send_packet_with_outcome(packet).await;

    assert_eq!(outcome, SendPacketOutcome::DroppedNoRoute);
}

#[tokio::test]
async fn send_packet_with_outcome_drops_announce_without_route() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &identity, false);
    let transport = Transport::new(config);

    let packet = Packet {
        header: Header { packet_type: PacketType::Announce, ..Default::default() },
        destination: AddressHash::new_from_rand(OsRng),
        ..Default::default()
    };
    let outcome = transport.send_packet_with_outcome(packet).await;

    assert_eq!(outcome, SendPacketOutcome::DroppedNoRoute);
}

#[tokio::test]
async fn duplicate_filter_allows_repeated_resource_requests() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &identity, false);
    let transport = Transport::new(config);
    let handler = transport.get_handler();

    let packet = Packet {
        header: Header {
            destination_type: DestinationType::Link,
            packet_type: PacketType::Data,
            ..Default::default()
        },
        context: PacketContext::ResourceRequest,
        destination: AddressHash::new_from_rand(OsRng),
        data: PacketDataBuffer::new_from_slice(b"same resource request"),
        ..Default::default()
    };

    assert!(handler.lock().await.filter_duplicate_packets(&packet).await);
    assert!(handler.lock().await.filter_duplicate_packets(&packet).await);
}

#[tokio::test]
async fn resource_request_responses_use_bound_link_iface_without_route_lookup() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &local_identity, false);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let mut iface_channel = transport.iface_manager.lock().await.new_channel(8);
    let iface = iface_channel.address;

    let remote_signer = PrivateIdentity::new_from_rand(OsRng);
    let remote_identity = *remote_signer.as_identity();
    let destination = DestinationDesc {
        identity: remote_identity,
        address_hash: remote_identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    let (link_events, _) = tokio::sync::broadcast::channel(4);
    let mut outbound = Link::new(destination, link_events.clone());
    let request_packet = outbound.request();
    let mut inbound = Link::new_from_request(
        &request_packet,
        remote_signer.sign_key().clone(),
        destination,
        link_events,
    )
    .expect("inbound link");
    let proof = inbound.prove();
    let _ = outbound.handle_packet(&proof, iface);
    let link_id = *outbound.id();

    let advertisement_packet = {
        let mut guard = handler.lock().await;
        let link = Arc::new(Mutex::new(outbound));
        guard.out_links.insert(destination.address_hash, link.clone());
        let link_guard = link.lock().await;
        let (resource_hash, packet) = guard
            .resource_manager
            .start_send(&link_guard, vec![0x42; PACKET_MDU + 24], None)
            .expect("start resource");
        guard.resource_manager.confirm_outbound_dispatch(resource_hash, true);
        packet
    };

    let link = handler
        .lock()
        .await
        .out_links
        .get(&destination.address_hash)
        .cloned()
        .expect("outbound link");
    let link_guard = link.lock().await;
    let advertisement = decrypt_resource_advertisement(&link_guard, &advertisement_packet);
    let requested_hashes = advertisement
        .hashmap
        .as_chunks::<MAPHASH_LEN>()
        .0
        .iter()
        .map(|chunk| {
            let mut hash = [0u8; MAPHASH_LEN];
            hash.copy_from_slice(chunk);
            hash
        })
        .collect::<Vec<_>>();
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: advertisement.hash,
        requested_hashes,
    };
    let resource_request_packet = encrypted_resource_control_packet(
        &link_guard,
        PacketContext::ResourceRequest,
        &request.encode(),
    );
    drop(link_guard);

    handle_data(&resource_request_packet, iface, handler.lock().await).await;

    let sent = timeout(Duration::from_millis(200), iface_channel.tx_channel.recv())
        .await
        .expect("resource parts should be sent on bound iface")
        .expect("tx channel open");
    assert_eq!(sent.tx_type, TxMessageType::Direct(iface));
    assert_eq!(sent.packet.destination, link_id);
    assert_eq!(sent.packet.context, PacketContext::Resource);
}

#[tokio::test]
async fn resource_request_responses_fit_bound_iface_mtu() {
    const LORA_MTU: usize = 220;

    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let config = TransportConfig::new("test", &local_identity, false);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let mut iface_channel = transport.iface_manager.lock().await.new_channel_with_role_mode_mtu(
        8,
        crate::iface::IfaceRole::Unicast,
        crate::iface::InterfaceMode::Full,
        LORA_MTU,
    );
    let iface = iface_channel.address;

    let remote_signer = PrivateIdentity::new_from_rand(OsRng);
    let remote_identity = *remote_signer.as_identity();
    let destination = DestinationDesc {
        identity: remote_identity,
        address_hash: remote_identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    let (link_events, _) = tokio::sync::broadcast::channel(4);
    let mut outbound = Link::new(destination, link_events.clone());
    let request_packet = outbound.request();
    let mut inbound = Link::new_from_request(
        &request_packet,
        remote_signer.sign_key().clone(),
        destination,
        link_events,
    )
    .expect("inbound link");
    let proof = inbound.prove();
    let _ = outbound.handle_packet(&proof, iface);
    let link_id = *outbound.id();

    let advertisement_packet = {
        let mut guard = handler.lock().await;
        let link = Arc::new(Mutex::new(outbound));
        guard.out_links.insert(destination.address_hash, link.clone());
        let link_guard = link.lock().await;
        // Random, not a repeated byte: this test's whole point is
        // multiple constrained-MTU parts, which auto-compression would
        // otherwise collapse a uniform-byte payload down below.
        let mut payload = vec![0u8; PACKET_MDU * 2];
        OsRng.fill_bytes(&mut payload);
        let (resource_hash, packet) = guard
            .resource_manager
            .start_send_with_mtu(&link_guard, payload, None, LORA_MTU)
            .expect("start resource");
        guard.resource_manager.confirm_outbound_dispatch(resource_hash, true);
        packet
    };

    let link = handler
        .lock()
        .await
        .out_links
        .get(&destination.address_hash)
        .cloned()
        .expect("outbound link");
    let link_guard = link.lock().await;
    let advertisement = decrypt_resource_advertisement(&link_guard, &advertisement_packet);
    assert!(advertisement.parts > 1, "test payload should require multiple constrained-MTU parts");
    let requested_hashes = advertisement
        .hashmap
        .as_chunks::<MAPHASH_LEN>()
        .0
        .iter()
        .map(|chunk| {
            let mut hash = [0u8; MAPHASH_LEN];
            hash.copy_from_slice(chunk);
            hash
        })
        .collect::<Vec<_>>();
    let requested_count = requested_hashes.len();
    assert!(requested_count > 0, "advertisement must offer at least one resource part");
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash: advertisement.hash,
        requested_hashes,
    };
    let resource_request_packet = encrypted_resource_control_packet(
        &link_guard,
        PacketContext::ResourceRequest,
        &request.encode(),
    );
    drop(link_guard);

    handle_data(&resource_request_packet, iface, handler.lock().await).await;

    for _ in 0..requested_count {
        let sent = timeout(Duration::from_millis(200), iface_channel.tx_channel.recv())
            .await
            .expect("resource part should be sent on bound iface")
            .expect("tx channel open");
        assert_eq!(sent.tx_type, TxMessageType::Direct(iface));
        assert_eq!(sent.packet.destination, link_id);
        assert_eq!(sent.packet.context, PacketContext::Resource);
        let wire_len = sent.packet.to_bytes().expect("serialize resource part").len();
        assert!(
            wire_len <= LORA_MTU,
            "resource part serialized to {wire_len} bytes, exceeding MTU {LORA_MTU}"
        );
    }
}

include!("resource_advertisement_test_helper.rs");
