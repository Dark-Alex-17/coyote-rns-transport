// Test code holds guards across awaits on purpose; production code is lint-clean.
#![allow(clippy::await_holding_invalid_type)]

use super::*;
use crate::iface::InterfaceMode;
use rand_core::OsRng;

fn message(packet_type: PacketType, destination: AddressHash) -> RxMessage {
    RxMessage {
        address: AddressHash::new_from_rand(OsRng),
        packet: Packet {
            header: crate::packet::Header { packet_type, ..Default::default() },
            destination,
            ..Default::default()
        },
        source: IfaceSource::None,
    }
}

async fn processed_hops(transport: &Transport, iface: AddressHash, wire_hops: u8) -> u8 {
    let mut inbound = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    inbound.address = iface;
    inbound.packet.header.hops = wire_hops;
    preprocess_inbound_message(&transport.get_handler(), &transport.iface_messages_tx, inbound)
        .await
        .expect("inbound packet should be queued")
        .1
        .message
        .packet
        .header
        .hops
}

#[test]
fn rns_1_5_ingress_classifies_management_traffic_before_queueing() {
    let path_request = AddressHash::new_from_rand(OsRng);
    let ordinary = AddressHash::new_from_rand(OsRng);

    assert_eq!(
        inbound_traffic_class(&message(PacketType::Data, ordinary), path_request),
        InboundTrafficClass::Data
    );
    assert_eq!(
        inbound_traffic_class(&message(PacketType::Announce, ordinary), path_request),
        InboundTrafficClass::Announce
    );
    assert_eq!(
        inbound_traffic_class(&message(PacketType::Data, path_request), path_request),
        InboundTrafficClass::PathRequest
    );
}

#[tokio::test]
async fn rns_1_5_ingress_plain_and_group_hop_filter_uses_wire_hops() {
    let transport = Transport::new(TransportConfig::default());
    let handler = transport.get_handler();
    let iface = *transport.iface_manager().lock().await.new_channel(8).address();

    for destination_type in [DestinationType::Plain, DestinationType::Group] {
        for wire_hops in [0, 1] {
            let mut inbound = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
            inbound.address = iface;
            inbound.packet.header.destination_type = destination_type;
            inbound.packet.header.hops = wire_hops;
            assert!(
                preprocess_inbound_message(&handler, &transport.iface_messages_tx, inbound)
                    .await
                    .is_some(),
                "wire hops {wire_hops} must be accepted for {destination_type:?}"
            );
        }

        let mut transported = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
        transported.address = iface;
        transported.packet.header.destination_type = destination_type;
        transported.packet.header.hops = 2;
        assert!(
            preprocess_inbound_message(&handler, &transport.iface_messages_tx, transported,)
                .await
                .is_none(),
            "wire hops 2 must be rejected for {destination_type:?}"
        );
    }
}

#[tokio::test]
async fn shared_instance_receive_removes_only_the_local_boundary_hop() {
    let mut config = TransportConfig::default();
    config.set_connected_to_shared_instance(true);
    let transport = Transport::new(config);
    let (shared_iface, ordinary_iface) = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        let shared_iface = *manager.new_channel(8).address();
        let ordinary_iface = *manager.new_channel(8).address();
        assert!(manager.set_shared_instance(shared_iface, true));
        (shared_iface, ordinary_iface)
    };

    assert_eq!(processed_hops(&transport, shared_iface, 0).await, 0);
    assert_eq!(processed_hops(&transport, shared_iface, 1).await, 1);
    assert_eq!(processed_hops(&transport, ordinary_iface, 0).await, 1);
}

#[tokio::test]
async fn shared_instance_parent_does_not_remove_child_boundary_hop() {
    let mut config = TransportConfig::default();
    config.set_connected_to_shared_instance(true);
    let transport = Transport::new(config);
    let (parent, child) = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        let parent = *manager.new_channel(8).address();
        let child = *manager.new_channel(8).address();
        assert!(manager.set_shared_instance(parent, true));
        assert!(manager.inherit_runtime_config(parent, child));
        (parent, child)
    };

    assert_eq!(processed_hops(&transport, parent, 0).await, 1);
    assert_eq!(processed_hops(&transport, child, 0).await, 0);
}

#[deny(clippy::await_holding_invalid_type)]
async fn filter_twice(
    connected_to_shared_instance: bool,
) -> (Arc<Mutex<PacketCache>>, (bool, bool), (bool, bool)) {
    let packet_cache = Arc::new(Mutex::new(PacketCache::new()));
    let packet = message(PacketType::Data, AddressHash::new_from_rand(OsRng)).packet;
    let transport_identity = AddressHash::new_from_rand(OsRng);
    let mut results = Vec::new();
    for _ in 0..2 {
        results.push(
            filter_duplicate_packet(
                packet_cache.clone(),
                None,
                "test",
                &packet,
                connected_to_shared_instance,
                transport_identity,
            )
            .await,
        );
    }
    (packet_cache, results[0], results[1])
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_client_filter_inserts_nothing_into_the_packet_cache() {
    let (packet_cache, first, second) = filter_twice(true).await;

    assert_eq!(first, (true, false), "first packet must be accepted without a cache insert");
    assert_eq!(second, (true, false), "repeat must be accepted without a cache insert");
    assert!(packet_cache.lock().await.hashes().is_empty(), "owner dedups; local cache stays empty");
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn standalone_transport_still_populates_the_packet_cache() {
    let (packet_cache, first, second) = filter_twice(false).await;

    assert_eq!(first, (true, true), "first packet must be accepted and inserted");
    assert_eq!(second, (false, false), "repeat must be dropped as a duplicate");
    assert_eq!(packet_cache.lock().await.hashes().len(), 1);
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_client_accepts_the_same_packet_twice() {
    let mut config = TransportConfig::default();
    config.set_connected_to_shared_instance(true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let iface = *transport.iface_manager().lock().await.new_channel(8).address();
    let mut inbound = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    inbound.address = iface;

    for attempt in 1..=2 {
        let (_, queued) =
            preprocess_inbound_message(&handler, &transport.iface_messages_tx, inbound.clone())
                .await
                .unwrap_or_else(|| panic!("attempt {attempt} must be accepted"));
        assert!(!queued.packet_cache_inserted, "attempt {attempt} must not insert into the cache");
    }
}

/// README `## Attaching to a shared instance` and its "Known divergences" entry: the
/// reverse-routing source note `handle_data` leaves for a Data packet received while
/// attached shares the cache with the duplicate filter, so the same packet is treated as
/// seen once the transport detaches, while a packet never received is still accepted.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_client_data_received_while_attached_is_seen_after_detach() {
    let mut config = TransportConfig::default();
    config.set_connected_to_shared_instance(true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let iface = *transport.iface_manager().lock().await.new_channel(8).address();
    let mut received = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    received.address = iface;
    let mut unseen = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    unseen.address = iface;

    let (_, queued) =
        preprocess_inbound_message(&handler, &transport.iface_messages_tx, received.clone())
            .await
            .expect("the packet is accepted while attached");
    assert!(!queued.packet_cache_inserted, "the filter inserts nothing while attached");
    process_inbound_message(handler.clone(), queued).await;
    let packet_cache = handler.lock().await.packet_cache.clone();
    let noted = packet_cache.lock().await.hashes().len();
    assert_eq!(noted, 1, "handle_data leaves the source note in the shared cache");

    transport.set_connected_to_shared_instance(false).await;

    assert!(
        preprocess_inbound_message(&handler, &transport.iface_messages_tx, received)
            .await
            .is_none(),
        "a Data packet received while attached counts as seen once detached"
    );
    let (_, fresh) = preprocess_inbound_message(&handler, &transport.iface_messages_tx, unseen)
        .await
        .expect("a packet never received is still accepted after the detach");
    assert!(fresh.packet_cache_inserted, "the detached filter is back to inserting");
}

#[tokio::test]
async fn rns_1_5_ingress_full_queue_does_not_poison_packet_retry() {
    let transport = Transport::new(TransportConfig::default());
    let handler = transport.get_handler();
    let iface = *transport.iface_manager().lock().await.new_channel(8).address();
    let queues = InboundQueues::new(InboundQueueLimits {
        data: 1,
        announce: 1,
        path_request: 1,
        ingress_limited: 1,
    });

    let mut first = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    first.address = iface;
    let (first_class, first) =
        preprocess_inbound_message(&handler, &transport.iface_messages_tx, first)
            .await
            .expect("first packet");
    queues.enqueue(first_class, first).expect("queue first packet");

    let mut retry = message(PacketType::Data, AddressHash::new_from_rand(OsRng));
    retry.address = iface;
    retry.packet.data = PacketDataBuffer::new_from_slice(b"retry after queue pressure");
    let (retry_class, rejected) =
        preprocess_inbound_message(&handler, &transport.iface_messages_tx, retry.clone())
            .await
            .expect("first retry attempt");
    let full = queues.enqueue(retry_class, rejected).expect_err("data queue must be full");
    rollback_rejected_inbound(&handler, &full.item).await;
    let _ = queues.try_dequeue().expect("free queue capacity");

    assert!(
        preprocess_inbound_message(&handler, &transport.iface_messages_tx, retry).await.is_some(),
        "queue-full rejection must not poison the packet hash cache"
    );
}

#[tokio::test]
async fn rns_1_5_ingress_path_request_deduplication_reaches_the_scoped_cache() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let mut config = TransportConfig::new("test", &local_identity, true);
    config.set_transport_enabled(true);
    let transport = Transport::new(config);
    let handler = transport.get_handler();
    let path_request_destination = handler.lock().await.fixed_dest_path_requests;

    let (iface_a_channel, iface_b_channel) = {
        let manager = transport.iface_manager();
        let mut manager = manager.lock().await;
        (
            manager.new_channel_with_role_and_mode(
                16,
                IfaceRole::Unicast,
                InterfaceMode::AccessPoint,
            ),
            manager.new_channel_with_role_and_mode(
                16,
                IfaceRole::Unicast,
                InterfaceMode::AccessPoint,
            ),
        )
    };
    let iface_a = *iface_a_channel.address();
    let iface_b = *iface_b_channel.address();

    let destination = AddressHash::new_from_rand(OsRng);
    let mut generator = PathRequests::new("", None, 16, 16, 30);

    let first_packet =
        generator.generate(&destination, Some(vec![0x5a; crate::hash::ADDRESS_HASH_SIZE]));
    let queued = preprocess_inbound_message(
        &handler,
        &transport.iface_messages_tx,
        RxMessage { address: iface_a, packet: first_packet, source: IfaceSource::None },
    )
    .await
    .expect("first path request must be queued");
    assert_eq!(queued.0, InboundTrafficClass::PathRequest);
    process_inbound_message(handler.clone(), queued.1).await;

    let second_packet =
        generator.generate(&destination, Some(vec![0x5b; crate::hash::ADDRESS_HASH_SIZE]));
    assert!(
        preprocess_inbound_message(
            &handler,
            &transport.iface_messages_tx,
            RxMessage { address: iface_b, packet: second_packet, source: IfaceSource::None },
        )
        .await
        .is_none(),
        "same-destination request must batch before entering the queue"
    );

    let guard = handler.lock().await;
    assert_eq!(guard.iface_manager.lock().await.mode(&iface_a), Some(InterfaceMode::AccessPoint));
    drop(guard);

    assert_eq!(
        handler.lock().await.path_requests.discovery_requesters(&destination),
        vec![iface_a, iface_b]
    );
    assert_eq!(generator.generate(&destination, None).destination, path_request_destination);
}
