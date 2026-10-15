// `in_links` is keyed by link id, `out_links` by destination hash. The resource-retry
// worker only has the link id, so an initiator-side link must still resolve to its bound
// interface or every retry from the initiator falls back to path routing. The maps are
// owned by the test, so the lookup runs without a handler lock.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resource_retry_link_lookup_resolves_both_link_directions() {
    let signer = PrivateIdentity::new_from_rand(OsRng);
    let identity = *signer.as_identity();
    let destination = DestinationDesc {
        identity,
        address_hash: identity.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    };
    // Both halves of one pair share a link id, so each direction gets its own pair.
    let (mut outbound, _) = activated_link_pair(destination, &signer);
    let out_iface = AddressHash::new_from_rand(OsRng);
    outbound.set_ingress_iface(out_iface);
    let out_link_id = *outbound.id();
    let (_, mut inbound) = activated_link_pair(destination, &signer);
    let in_iface = AddressHash::new_from_rand(OsRng);
    inbound.set_ingress_iface(in_iface);
    let in_link_id = *inbound.id();

    let mut out_links = HashMap::new();
    out_links.insert(destination.address_hash, Arc::new(Mutex::new(outbound)));
    let mut in_links = HashMap::new();
    in_links.insert(in_link_id, Arc::new(Mutex::new(inbound)));

    let outbound = super::resource_wire::link_by_id(&in_links, &out_links, &out_link_id)
        .await
        .expect("initiator-side link found by id although out_links is keyed by destination hash");
    assert_eq!(outbound.lock().await.ingress_iface(), Some(out_iface));

    let inbound = super::resource_wire::link_by_id(&in_links, &out_links, &in_link_id)
        .await
        .expect("listener-side link found by id");
    assert_eq!(inbound.lock().await.ingress_iface(), Some(in_iface));

    let unknown = AddressHash::new_from_rand(OsRng);
    assert!(
        super::resource_wire::link_by_id(&in_links, &out_links, &unknown).await.is_none(),
        "unknown link id resolves to no link"
    );
}
