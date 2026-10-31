// The per-link `ProofStrategy`: whether a plain Data packet received on the
// link is proved on receipt, as Python's `PROVE_ALL`/`PROVE_APP` arms decide
// (`RNS/Link.py:961-968`), and the hash-keyed `prove_hash` an application
// uses to prove later. `include!`d into the same module.
#[cfg(test)]
mod proof_strategy_tests {
    use super::data_proof_tests::packet_proven_events;
    use super::identify_packet_tests::linked_pair;
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::TryRecvError;

    /// A callback that answers `verdict` and counts how often it was asked.
    fn counting_callback(verdict: bool) -> (Box<dyn ProofRequestedHandler>, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let callback = move |_: &Packet| {
            counter.fetch_add(1, Ordering::SeqCst);
            verdict
        };
        (Box::new(callback), calls)
    }

    /// The payload of the first `Data` event on `rx`, skipping everything else.
    fn first_data_payload(rx: &mut broadcast::Receiver<LinkEventData>) -> LinkPayload {
        loop {
            match rx.try_recv() {
                Ok(LinkEventData { event: LinkEvent::Data(payload), .. }) => return *payload,
                Ok(_) | Err(TryRecvError::Lagged(_)) => continue,
                Err(TryRecvError::Empty | TryRecvError::Closed) => panic!("no Data event posted"),
            }
        }
    }

    #[test]
    fn both_constructors_default_to_all() {
        let (outbound, inbound, _, _) = linked_pair();
        assert_eq!(outbound.proof_strategy(), ProofStrategy::All);
        assert_eq!(inbound.proof_strategy(), ProofStrategy::All);
    }

    /// Under `App` with no callback the packet is delivered but not proved; the posted
    /// payload carries the packet hash, and a `prove_hash` of that hash fed back to the
    /// sender posts exactly one `PacketProven` for it.
    #[test]
    fn app_without_callback_defers_the_proof_to_prove_hash() {
        let (mut outbound, mut inbound, iface, mut rx) = linked_pair();
        inbound.set_proof_strategy(ProofStrategy::App);
        let data = outbound.data_packet(b"prove me later").expect("data packet");
        let hash = data.hash();
        outbound.note_data_sent(hash);

        assert!(matches!(inbound.handle_packet(&data, iface), LinkHandleResult::None));
        let payload = first_data_payload(&mut rx);
        assert_eq!(payload.as_slice(), b"prove me later");
        assert_eq!(payload.packet_hash(), Some(hash));
        assert_eq!(packet_proven_events(&mut rx), vec![]);

        let proof = inbound.prove_hash(&hash);
        assert!(matches!(outbound.handle_packet(&proof, iface), LinkHandleResult::None));
        assert_eq!(packet_proven_events(&mut rx), vec![hash]);
    }

    #[test]
    fn app_with_a_consenting_callback_proves_on_receipt() {
        let (outbound, mut inbound, iface, _) = linked_pair();
        let (callback, calls) = counting_callback(true);
        inbound.set_proof_strategy(ProofStrategy::App);
        inbound.set_proof_requested_callback(callback);
        let data = outbound.data_packet(b"ask first").expect("data packet");

        assert!(matches!(inbound.handle_packet(&data, iface), LinkHandleResult::Proof(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn app_with_a_refusing_callback_is_consulted_and_does_not_prove() {
        let (outbound, mut inbound, iface, _) = linked_pair();
        let (callback, calls) = counting_callback(false);
        inbound.set_proof_strategy(ProofStrategy::App);
        inbound.set_proof_requested_callback(callback);
        let data = outbound.data_packet(b"ask first").expect("data packet");

        assert!(matches!(inbound.handle_packet(&data, iface), LinkHandleResult::None));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn none_never_proves_and_never_asks_the_callback() {
        let (outbound, mut inbound, iface, mut rx) = linked_pair();
        let (callback, calls) = counting_callback(true);
        inbound.set_proof_strategy(ProofStrategy::None);
        inbound.set_proof_requested_callback(callback);
        let data = outbound.data_packet(b"silence").expect("data packet");

        assert!(matches!(inbound.handle_packet(&data, iface), LinkHandleResult::None));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(first_data_payload(&mut rx).as_slice(), b"silence");
    }

    #[test]
    fn all_proves_without_asking_the_callback() {
        let (outbound, mut inbound, iface, _) = linked_pair();
        let (callback, calls) = counting_callback(false);
        inbound.set_proof_requested_callback(callback);
        let data = outbound.data_packet(b"always").expect("data packet");

        assert!(matches!(inbound.handle_packet(&data, iface), LinkHandleResult::Proof(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    /// A Channel frame is proved under every strategy (`RNS/Link.py:1151`).
    #[test]
    fn channel_frames_are_proved_under_app_and_none() {
        for strategy in [ProofStrategy::App, ProofStrategy::None] {
            let (mut outbound, mut inbound, iface, _) = linked_pair();
            inbound.set_proof_strategy(strategy);
            inbound.register_channel_handler(0x55AA, |_| true);
            let (_, packet) = outbound
                .send_channel_message(0x55AA, b"channel frame".to_vec())
                .expect("channel message");

            assert!(
                matches!(inbound.handle_packet(&packet, iface), LinkHandleResult::Proof(_)),
                "a channel frame is proved under {strategy:?}"
            );
        }
    }

    #[test]
    fn prove_hash_builds_the_same_proof_as_prove_packet() {
        let (outbound, inbound, _, _) = linked_pair();
        let data = outbound.data_packet(b"twins").expect("data packet");

        let by_packet = inbound.prove_packet(&data);
        let by_hash = inbound.prove_hash(&data.hash());

        assert_eq!(by_hash.header, by_packet.header);
        assert_eq!(by_hash.destination, by_packet.destination);
        assert_eq!(by_hash.context, by_packet.context);
        assert_eq!(by_hash.data.as_slice(), by_packet.data.as_slice());
    }
}
