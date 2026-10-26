// `send_prepared_packet_broadcast_with_trace` hands a packet straight to the interfaces and
// skips the transport's send bookkeeping for every destination type. That is what its caller
// wants for pre-encrypted opportunistic traffic, whose proofs it matches itself by the hash in
// the trace, and it is why the API is unfit for anything the transport should track: a packet
// sent this way leaves no packet-cache entry, so nothing on this side will later match a proof
// or a duplicate against it. These tests pin that contract for a link packet, next to the link
// send path that does record the hash, and for a Single packet built the way the opportunistic
// caller builds one. Helpers come from the sibling `tests_parts` files.

#[deny(clippy::await_holding_invalid_type)]
async fn packet_cache_holds(transport: &Transport, hash: &Hash) -> bool {
    let packet_cache = transport.get_handler().lock().await.packet_cache.clone();
    let hashes = packet_cache.lock().await.hashes();
    hashes.contains(hash)
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn prepared_broadcast_does_not_record_a_link_packet() {
    let pair = loopback_link_pair("prepared-broadcast").await;
    let transport = &pair.connector.transport;
    let link = transport.find_out_link(&pair.link_id).await.expect("connector holds the link");

    let prepared = link.lock().await.data_packet(b"prepared").expect("plain data fits a packet");
    let prepared_hash = prepared.hash();
    let trace = transport.send_prepared_packet_broadcast_with_trace(prepared).await;
    assert!(trace.outcome.is_sent(), "the broadcast reaches the connector's interface");
    assert_eq!(trace.packet_hash, Some(prepared_hash));
    assert!(
        !packet_cache_holds(transport, &prepared_hash).await,
        "a prepared broadcast is not recorded, so no proof or duplicate bookkeeping applies"
    );

    let recorded = link.lock().await.data_packet(b"recorded").expect("plain data fits a packet");
    let recorded_hash = recorded.hash();
    let outcome = transport.send_link_packet_on_bound_iface(&link, recorded).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the link send reaches the wire");
    assert!(
        packet_cache_holds(transport, &recorded_hash).await,
        "the link send path records the hash the peer will prove"
    );
}

/// The Single-destination twin of the link test. The function cannot fail this test without
/// also recording link packets, so it documents the contract rather than guarding a fix.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn prepared_broadcast_does_not_record_a_single_packet() {
    let pair = loopback_link_pair("prepared-broadcast-single").await;
    let transport = &pair.connector.transport;

    let prepared = Packet {
        header: Header {
            header_type: HeaderType::Type1,
            propagation_type: crate::packet::PropagationType::Broadcast,
            destination_type: DestinationType::Single,
            packet_type: PacketType::Data,
            ..Header::default()
        },
        destination: AddressHash::new_from_rand(OsRng),
        data: PacketDataBuffer::new_from_slice(b"opportunistic ciphertext"),
        ..Packet::default()
    };
    let prepared_hash = prepared.hash();
    let trace = transport.send_prepared_packet_broadcast_with_trace(prepared).await;
    assert!(trace.outcome.is_sent(), "the broadcast reaches the connector's interface");
    assert_eq!(trace.packet_hash, Some(prepared_hash));
    assert!(
        !packet_cache_holds(transport, &prepared_hash).await,
        "a prepared Single broadcast is not recorded either; its caller matches the proof"
    );
}
