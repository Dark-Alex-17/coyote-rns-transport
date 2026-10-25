/// An outgoing resource on `link_a`, advertised and confirmed, with a second link
/// that knows nothing about it.
fn outgoing_on_link_a() -> (ResourceManager, Link, Link, Hash) {
    let (mut manager, link_a) = manager_with_test_link();
    let link_b = second_test_link();
    let (resource_hash, _) =
        manager.start_send(&link_a, b"served on link a only".to_vec(), None).expect("start sender");
    manager.confirm_outbound_dispatch(resource_hash, true);
    (manager, link_a, link_b, resource_hash)
}

/// A resource request arriving on one link names a sender that belongs to another
/// link: no parts are served to it, while the same request on the sender's own link is.
#[test]
fn resource_manager_serves_no_parts_to_a_request_from_another_link() {
    let (mut manager, mut link_a, mut link_b, resource_hash) = outgoing_on_link_a();
    let first_map_hash = manager.outgoing[&resource_hash].map_hashes[0];
    let request = ResourceRequest {
        hashmap_exhausted: false,
        last_map_hash: None,
        resource_hash,
        requested_hashes: vec![first_map_hash],
    };
    let encoded = request.encode();

    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &encoded, *link_b.id()),
        &mut link_b,
    );
    assert!(responses.is_empty(), "another link's request earns no parts: {responses:?}");

    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceRequest, &encoded, *link_a.id()),
        &mut link_a,
    );
    assert_eq!(responses.len(), 1, "the request on its own link is served");
    assert_eq!(responses[0].context, PacketContext::Resource);
}

/// A resource proof arriving on one link names a sender that belongs to another link:
/// the sender stays in flight and nothing concludes, while the same proof on the
/// sender's own link completes it.
#[test]
fn resource_manager_ignores_a_proof_from_another_link() {
    let (mut manager, mut link_a, mut link_b, resource_hash) = outgoing_on_link_a();
    let expected_proof = manager.outgoing[&resource_hash].expected_proof;
    let proof = ResourceProof { resource_hash, proof: expected_proof }.encode();

    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof, *link_b.id()),
        &mut link_b,
    );
    assert!(responses.is_empty());
    assert!(
        manager.outgoing.contains_key(&resource_hash),
        "another link's proof leaves the sender"
    );
    assert!(manager.drain_events().is_empty(), "another link's proof concludes nothing");

    let responses = manager.handle_packet(
        &resource_packet(PacketContext::ResourceProof, &proof, *link_a.id()),
        &mut link_a,
    );
    assert!(responses.is_empty());
    assert!(!manager.outgoing.contains_key(&resource_hash), "the proof on its own link concludes");
    let events = manager.drain_events();
    assert!(
        events.iter().any(|event| event.hash == resource_hash
            && event.link_id == *link_a.id()
            && matches!(event.kind, ResourceEventKind::OutboundComplete)),
        "got {events:?}"
    );
}
