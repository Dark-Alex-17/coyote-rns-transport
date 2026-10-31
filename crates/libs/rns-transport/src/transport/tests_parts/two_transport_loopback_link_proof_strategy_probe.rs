// The edges of the per-link `ProofStrategy`: a `SingleInputDestination` reports `All` as
// its `link_proof_strategy` until told otherwise and copies it at accept only, so a link
// already accepted keeps proving after the destination is switched to `App` while a link
// accepted afterwards starts out `App`; an out-link, which no destination accepts, under
// `App` proves nothing until `Link::set_proof_requested_callback` gives it a consenting
// callback; and `Transport::prove_link_packet` under `All` is `Ok` yet earns the sender no
// second `PacketProven`, its one recorded hash already consumed (`RNS/Transport.py:2748-2761`).
// Helpers come from `two_transport_loopback_link_proof_strategy.rs` and its siblings.

/// The destination answers `All` before anything is set; the link it accepted under `All`
/// keeps `All` and keeps proving after the destination is switched to `App`; a link a second
/// connector opens afterwards is accepted as `App` and proves nothing by itself.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_probe_destination_default_is_copied_at_accept_only(
) {
    let listener = listener("proof-probe-copy-listener", TcpClient::DEFAULT_MTU).await;
    let mut first =
        connector("proof-probe-copy-first", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();
    let mut first_out_events = first.transport.out_link_events();

    let (destination, desc) =
        register_in_destination(&listener.transport, "loopback", "proof-probe-copy").await;
    assert_eq!(
        destination.lock().await.link_proof_strategy(),
        ProofStrategy::All,
        "a fresh destination hands its links the default strategy"
    );
    let learned = learn_announce(&mut first.announces, &desc.address_hash).await;
    let (first_link, first_id) = open_link(&first.transport, learned).await;
    await_event(&mut in_events, |event| {
        event.id == first_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;

    // Switching the destination now must not reach the link it already accepted.
    destination.lock().await.set_link_proof_strategy(ProofStrategy::App);
    assert_eq!(destination.lock().await.link_proof_strategy(), ProofStrategy::App);
    let first_in = listener
        .transport
        .find_in_link(&first_id)
        .await
        .expect("listener tracks the first inbound link");
    assert_eq!(
        first_in.lock().await.proof_strategy(),
        ProofStrategy::All,
        "a link already accepted keeps the strategy it was given"
    );
    let sent = packet_proven_send_plain_data(&first.transport, &first_link, b"still-all").await;
    assert_eq!(
        packet_proven_probe_collect(&mut first_out_events, first_id, 1).await,
        vec![sent],
        "the accepted-under-All link still proves on receipt"
    );

    // A link accepted from now on starts out `App`: re-announce so a second connector
    // learns the destination, open a link from it and send plain data over it.
    let mut second =
        connector("proof-probe-copy-second", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(2).await;
    let mut second_out_events = second.transport.out_link_events();
    let announce = destination.lock().await.announce(OsRng, None).expect("valid announce packet");
    listener.transport.send_packet(announce).await;
    let learned = learn_announce(&mut second.announces, &desc.address_hash).await;
    let (second_link, second_id) = open_link(&second.transport, learned).await;
    assert_ne!(first_id, second_id, "the second connector opens a link of its own");
    await_event(&mut in_events, |event| {
        event.id == second_id && matches!(event.event, LinkEvent::Activated)
    })
    .await;
    let second_in = listener
        .transport
        .find_in_link(&second_id)
        .await
        .expect("listener tracks the second inbound link");
    assert_eq!(
        second_in.lock().await.proof_strategy(),
        ProofStrategy::App,
        "a link accepted after the switch starts with the destination's new strategy"
    );
    let sent = packet_proven_send_plain_data(&second.transport, &second_link, b"now-app").await;
    let payload = await_plain_data(&mut in_events, second_id, b"now-app").await;
    assert_eq!(payload.packet_hash(), Some(sent));
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut second_out_events, second_id).await,
        0,
        "the App link proves nothing without a callback"
    );
}

/// An out-link set to `App` by hand proves nothing for the listener's plain Data packet
/// until the application gives it a consenting callback through
/// `Link::set_proof_requested_callback`; the held-back packet can still be proved through
/// `Transport::prove_link_packet` from the connector's side.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_probe_out_link_app_waits_for_its_callback() {
    let mut pair = proof_strategy_pair("proof-probe-out-app", ProofStrategy::All, None).await;
    let link_id = pair.link_id;
    pair.link.lock().await.set_proof_strategy(ProofStrategy::App);
    assert_eq!(pair.link.lock().await.proof_strategy(), ProofStrategy::App);
    let in_link = pair
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("listener tracks the inbound link");

    let held = packet_proven_send_plain_data(&pair.listener.transport, &in_link, b"held").await;
    let payload = await_plain_data(&mut pair.out_events, link_id, b"held").await;
    assert_eq!(payload.packet_hash(), Some(held));
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.in_events, link_id).await,
        0,
        "an out-link under App with no callback never proves by itself"
    );

    let (callback, calls) = counting_proof_callback(true);
    pair.link.lock().await.set_proof_requested_callback(callback);
    let asked = packet_proven_send_plain_data(&pair.listener.transport, &in_link, b"asked").await;
    assert_eq!(
        packet_proven_probe_collect(&mut pair.in_events, link_id, 1).await,
        vec![asked],
        "the consenting callback has the out-link prove on receipt"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the callback was asked once, for the new packet");

    pair.connector
        .transport
        .prove_link_packet(&link_id, &held)
        .await
        .expect("the deferred proof goes out on the out-link's interface");
    assert_eq!(
        packet_proven_probe_collect(&mut pair.in_events, link_id, 1).await,
        vec![held],
        "the held-back packet is proved by hand afterwards"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a by-hand proof does not consult the callback");
}

/// Under `All` the link already proved the packet on receipt; `prove_link_packet` is still
/// `Ok` but the sender, whose one recorded hash the first proof consumed, sees no second
/// `PacketProven` for it.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_probe_prove_link_packet_under_all_is_a_quiet_second_proof(
) {
    let mut pair = proof_strategy_pair("proof-probe-all-twice", ProofStrategy::All, None).await;
    let link_id = pair.link_id;

    let sent =
        packet_proven_send_plain_data(&pair.connector.transport, &pair.link, b"proved-twice").await;
    assert_eq!(packet_proven_probe_collect(&mut pair.out_events, link_id, 1).await, vec![sent]);

    pair.listener
        .transport
        .prove_link_packet(&link_id, &sent)
        .await
        .expect("a second proof under All still goes out");
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "the sender ignores a proof for a hash it no longer records"
    );
}
