// The per-packet proof of a plain Data packet sent on a link, matched against
// the packets this side recorded as sent and consumed by the first match
// (`RNS/Transport.py:2748-2761`). `include!`d into the same module.
#[cfg(test)]
mod data_proof_tests {
    use super::identify_packet_tests::linked_pair;
    use super::*;

    use tokio::sync::broadcast;
    use tokio::sync::broadcast::error::TryRecvError;

    pub(super) fn packet_proven_events(rx: &mut broadcast::Receiver<LinkEventData>) -> Vec<Hash> {
        let mut hashes = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(LinkEventData { event: LinkEvent::PacketProven { packet_hash }, .. }) => {
                    hashes.push(packet_hash);
                }
                Ok(_) | Err(TryRecvError::Lagged(_)) => continue,
                Err(TryRecvError::Empty | TryRecvError::Closed) => break,
            }
        }
        hashes
    }

    /// The first valid proof of a recorded packet posts the event and consumes the record,
    /// so the same proof arriving again posts nothing.
    #[test]
    fn a_recorded_data_packet_is_proven_once() {
        let (mut outbound, inbound, iface, mut rx) = linked_pair();
        let packet = outbound.data_packet(b"prove me").expect("data packet");
        let hash = packet.hash();
        outbound.note_data_sent(hash);
        let proof = inbound.prove_packet(&packet);

        assert!(matches!(outbound.handle_packet(&proof, iface), LinkHandleResult::None));
        assert_eq!(packet_proven_events(&mut rx), vec![hash]);

        assert!(matches!(outbound.handle_packet(&proof, iface), LinkHandleResult::None));
        assert_eq!(packet_proven_events(&mut rx), vec![], "a replayed proof posts nothing");
        assert!(outbound.unproven_data_hashes.is_empty());
    }

    /// A valid proof for a hash this side never recorded as sent posts nothing.
    #[test]
    fn a_proof_for_a_hash_this_side_never_sent_posts_nothing() {
        let (mut outbound, inbound, iface, mut rx) = linked_pair();
        let packet = outbound.data_packet(b"never sent").expect("data packet");
        let proof = inbound.prove_packet(&packet);

        assert!(matches!(outbound.handle_packet(&proof, iface), LinkHandleResult::None));
        assert_eq!(packet_proven_events(&mut rx), vec![]);
    }

    /// A Channel frame's hash is never recorded, so its proof marks the frame `Delivered`
    /// through the channel machinery and posts no `PacketProven`.
    #[test]
    fn a_channel_frame_proof_posts_nothing_and_still_delivers_the_frame() {
        let (mut outbound, mut inbound, iface, mut rx) = linked_pair();
        inbound.register_channel_handler(0x55AA, |_| true);
        let (sequence, packet) = outbound
            .send_channel_message(0x55AA, b"needs-proof".to_vec())
            .expect("channel message");
        assert_eq!(outbound.channel_state(sequence), ChannelMessageState::Sent);

        let proof = match inbound.handle_packet(&packet, iface) {
            LinkHandleResult::Proof(proof) => proof,
            _ => panic!("channel packet should generate link proof"),
        };
        assert!(matches!(outbound.handle_packet(&proof, iface), LinkHandleResult::None));

        assert_eq!(outbound.channel_state(sequence), ChannelMessageState::Delivered);
        assert_eq!(packet_proven_events(&mut rx), vec![]);
    }

    /// Past the cap the oldest record goes first, so only the newest
    /// `UNPROVEN_DATA_HASHES_CAP` hashes can still be proven.
    #[test]
    fn the_oldest_unproven_hash_is_dropped_past_the_cap() {
        let (mut outbound, _, _, _) = linked_pair();
        let hash_for = |i: usize| {
            let mut bytes = [0u8; HASH_SIZE];
            bytes[0] = i as u8;
            bytes[1] = (i >> 8) as u8;
            Hash::new(bytes)
        };
        for i in 0..=UNPROVEN_DATA_HASHES_CAP {
            outbound.note_data_sent(hash_for(i));
        }
        assert_eq!(outbound.unproven_data_hashes.len(), UNPROVEN_DATA_HASHES_CAP);

        assert!(!outbound.take_unproven_data_hash(&hash_for(0)), "the oldest was dropped");
        assert!(outbound.take_unproven_data_hash(&hash_for(1)));
        assert_eq!(outbound.unproven_data_hashes.len(), UNPROVEN_DATA_HASHES_CAP - 1);
    }

    /// A restart begins a new session, so a proof for a packet of the old one must not match.
    #[test]
    fn a_restart_forgets_the_unproven_hashes() {
        let (mut outbound, _, _, _) = linked_pair();
        let packet = outbound.data_packet(b"old session").expect("data packet");
        outbound.note_data_sent(packet.hash());

        outbound.restart();

        assert!(outbound.unproven_data_hashes.is_empty());
    }

    #[test]
    fn a_close_forgets_the_unproven_hashes() {
        let (mut outbound, _, _, _) = linked_pair();
        let packet = outbound.data_packet(b"closing").expect("data packet");
        outbound.note_data_sent(packet.hash());

        outbound.close();

        assert!(outbound.unproven_data_hashes.is_empty());
    }
}
