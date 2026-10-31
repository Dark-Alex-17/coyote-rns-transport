// Loopback coverage for the per-link `ProofStrategy` and `Transport::prove_link_packet`: a
// destination's `link_proof_strategy` (and its `proof_requested_callback`) is copied onto the
// link it accepts, so under `App` the listener delivers a plain Data packet without proving
// it and the application proves later from `LinkPayload::packet_hash`, the way a Python
// application calls `packet.prove()` on the packet its callback got (`RNS/Packet.py:336-341`,
// `RNS/Link.py:961-968`); an out-link set to `None` by hand does the same from the connector's
// side; and a proof for an unknown link is refused. Helpers come from the sibling
// `tests_parts` files.

use crate::destination::ProofRequestedHandler;

/// A link pair whose listener destination had `strategy` (and `callback`) set before the
/// link formed, with both event streams subscribed before the link request went out.
struct ProofStrategyPair {
    listener: LoopbackListener,
    connector: LoopbackConnector,
    link: Arc<Mutex<Link>>,
    link_id: AddressHash,
    in_events: broadcast::Receiver<LinkEventData>,
    out_events: broadcast::Receiver<LinkEventData>,
}

#[deny(clippy::await_holding_invalid_type)]
async fn proof_strategy_pair(
    aspect: &str,
    strategy: ProofStrategy,
    callback: Option<Box<dyn ProofRequestedHandler>>,
) -> ProofStrategyPair {
    let listener = listener("proof-strategy-listener", TcpClient::DEFAULT_MTU).await;
    let mut connector =
        connector("proof-strategy-connector", listener.port, TcpClient::DEFAULT_MTU).await;
    listener.await_accepted_connections(1).await;
    let mut in_events = listener.transport.in_link_events();
    let out_events = connector.transport.out_link_events();

    let (destination, desc) =
        register_in_destination(&listener.transport, "loopback", aspect).await;
    {
        let mut destination = destination.lock().await;
        destination.set_link_proof_strategy(strategy);
        if let Some(callback) = callback {
            destination.set_proof_requested_callback(callback);
        }
    }
    let learned = learn_announce(&mut connector.announces, &desc.address_hash).await;
    let (link, link_id) = open_link(&connector.transport, learned).await;
    let activated =
        await_event(&mut in_events, |event| matches!(event.event, LinkEvent::Activated)).await;
    assert_eq!(activated.id, link_id, "both ends share one link id");
    ProofStrategyPair { listener, connector, link, link_id, in_events, out_events }
}

/// A callback that answers `verdict` and counts how often it was asked.
fn counting_proof_callback(verdict: bool) -> (Box<dyn ProofRequestedHandler>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let callback = move |_: &Packet| {
        counter.fetch_add(1, Ordering::SeqCst);
        verdict
    };
    (Box::new(callback), calls)
}

/// Waits for the plain Data event carrying `body` on `events` and returns its payload.
#[deny(clippy::await_holding_invalid_type)]
async fn await_plain_data(
    events: &mut broadcast::Receiver<LinkEventData>,
    link_id: AddressHash,
    body: &[u8],
) -> LinkPayload {
    let event = await_event(events, |event| {
        event.id == link_id
            && matches!(&event.event, LinkEvent::Data(payload)
                if payload.context() == PacketContext::None && payload.as_slice() == body)
    })
    .await;
    match event.event {
        LinkEvent::Data(payload) => *payload,
        _ => unreachable!("the predicate admits only Data events"),
    }
}

/// Under `App` with no callback the listener delivers the packet, with its hash on the
/// payload, and proves nothing by itself: the connector sees no `PacketProven` while the
/// application sits on it. `prove_link_packet` with that hash then earns exactly one.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_app_defers_the_proof_to_the_application() {
    let mut pair = proof_strategy_pair("proof-app", ProofStrategy::App, None).await;
    let link_id = pair.link_id;

    let sent =
        packet_proven_send_plain_data(&pair.connector.transport, &pair.link, b"hello-app").await;
    let payload = await_plain_data(&mut pair.in_events, link_id, b"hello-app").await;
    assert_eq!(payload.packet_hash(), Some(sent), "the payload names the packet to prove");
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "the link does not prove on the application's behalf"
    );

    pair.listener
        .transport
        .prove_link_packet(&link_id, &sent)
        .await
        .expect("the proof goes out on the link's interface");
    assert_eq!(packet_proven_probe_collect(&mut pair.out_events, link_id, 1).await, vec![sent]);
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await,
        0,
        "one application proof earns one PacketProven"
    );
}

/// The accepted link starts with the destination's `link_proof_strategy`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_is_copied_onto_the_accepted_link() {
    let pair = proof_strategy_pair("proof-copied", ProofStrategy::App, None).await;

    let in_link = pair
        .listener
        .transport
        .find_in_link(&pair.link_id)
        .await
        .expect("listener tracks the inbound link");
    assert_eq!(in_link.lock().await.proof_strategy(), ProofStrategy::App);
    assert_eq!(
        pair.link.lock().await.proof_strategy(),
        ProofStrategy::All,
        "the initiator's link is not accepted by a destination and keeps the default"
    );
}

/// Under `App` a callback answering `true` has the link prove on receipt.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_app_callback_consents() {
    let (callback, calls) = counting_proof_callback(true);
    let mut pair = proof_strategy_pair("proof-consent", ProofStrategy::App, Some(callback)).await;
    let link_id = pair.link_id;

    let sent =
        packet_proven_send_plain_data(&pair.connector.transport, &pair.link, b"ask-yes").await;
    assert_eq!(packet_proven_probe_collect(&mut pair.out_events, link_id, 1).await, vec![sent]);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the callback was asked once");
}

/// Under `App` a callback answering `false` is asked once and the packet stays unproved.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_app_callback_refuses() {
    let (callback, calls) = counting_proof_callback(false);
    let mut pair = proof_strategy_pair("proof-refuse", ProofStrategy::App, Some(callback)).await;
    let link_id = pair.link_id;

    packet_proven_send_plain_data(&pair.connector.transport, &pair.link, b"ask-no").await;
    await_plain_data(&mut pair.in_events, link_id, b"ask-no").await;
    assert_eq!(packet_proven_probe_count_after_quiet(&mut pair.out_events, link_id).await, 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the callback was asked once");
}

/// An out-link set to `None` by hand delivers the listener's plain Data packet without
/// proving it; the connector's `prove_link_packet` then proves it to the listener.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_none_on_the_out_link_defers_too() {
    let mut pair = proof_strategy_pair("proof-out-none", ProofStrategy::All, None).await;
    let link_id = pair.link_id;
    pair.link.lock().await.set_proof_strategy(ProofStrategy::None);
    let in_link = pair
        .listener
        .transport
        .find_in_link(&link_id)
        .await
        .expect("listener tracks the inbound link");

    let sent = packet_proven_send_plain_data(&pair.listener.transport, &in_link, b"pong").await;
    let payload = await_plain_data(&mut pair.out_events, link_id, b"pong").await;
    assert_eq!(payload.packet_hash(), Some(sent));
    assert_eq!(
        packet_proven_probe_count_after_quiet(&mut pair.in_events, link_id).await,
        0,
        "the out-link does not prove under None"
    );

    pair.connector
        .transport
        .prove_link_packet(&link_id, &sent)
        .await
        .expect("the proof goes out on the link's interface");
    assert_eq!(packet_proven_probe_collect(&mut pair.in_events, link_id, 1).await, vec![sent]);
}

/// A link id neither side of the transport tracks is an invalid argument, not a silent drop.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_link_proof_strategy_unknown_link_is_refused() {
    let pair = proof_strategy_pair("proof-unknown", ProofStrategy::All, None).await;

    let outcome = pair
        .connector
        .transport
        .prove_link_packet(&AddressHash::new_from_rand(OsRng), &Hash::new_from_slice(b"x"))
        .await;
    assert!(matches!(outcome, Err(RnsError::InvalidArgument)), "{outcome:?}");
}
