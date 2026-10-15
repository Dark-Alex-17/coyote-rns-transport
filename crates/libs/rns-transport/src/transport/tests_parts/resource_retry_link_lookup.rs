// `in_links` is keyed by link id, `out_links` by destination hash. The resource-retry
// worker only has the link id, so an initiator-side link must still resolve to its bound
// interface or every retry from the initiator falls back to path routing.

// Holds the handler guard across the link lock on purpose: that is the worker's own
// lock order (handler, then link), and the lookup borrows the handler while it awaits.
async fn retry_iface_for(transport: &Transport, link_id: &AddressHash) -> Option<AddressHash> {
    let handler = transport.get_handler();
    let handler = handler.lock().await;
    super::jobs::link_ingress_iface(&handler, link_id).await
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resource_retry_link_lookup_resolves_both_link_directions() {
    let local_identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("test", &local_identity, false));
    let channel =
        transport.iface_manager().lock().await.new_channel_with_role(8, IfaceRole::Unicast);
    let iface = *channel.address();

    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    // Both halves of one pair share a link id, so each direction gets its own pair.
    let (mut outbound, _) = activated_link_pair(destination, &signer);
    outbound.set_ingress_iface(iface);
    let out_link_id = *outbound.id();
    let (_, mut inbound) = activated_link_pair(destination, &signer);
    let in_iface = AddressHash::new_from_rand(OsRng);
    inbound.set_ingress_iface(in_iface);
    let in_link_id = *inbound.id();
    {
        let handler = transport.get_handler();
        let mut handler = handler.lock().await;
        handler.out_links.insert(destination.address_hash, Arc::new(Mutex::new(outbound)));
        handler.in_links.insert(in_link_id, Arc::new(Mutex::new(inbound)));
    }

    assert_eq!(
        retry_iface_for(&transport, &out_link_id).await,
        Some(iface),
        "initiator-side link found by id although out_links is keyed by destination hash"
    );
    assert_eq!(
        retry_iface_for(&transport, &in_link_id).await,
        Some(in_iface),
        "listener-side link found by id"
    );
    let unknown = AddressHash::new_from_rand(OsRng);
    assert_eq!(
        retry_iface_for(&transport, &unknown).await,
        None,
        "unknown link id resolves to no interface"
    );
}
