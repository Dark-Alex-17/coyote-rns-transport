// The application-driven proof of a received link packet, `include!`d into
// `links_parts/transport.rs` beside `resolve_destination.rs` so that section
// stays below the repository's 500-line limit; a file boundary, not a
// privacy one.

impl Transport {
    /// Proves the link packet with hash `packet_hash` to the peer on link
    /// `link_id`: the "accept first, prove later" path for an application
    /// that reads the hash off `LinkPayload::packet_hash` when the
    /// `LinkEvent::Data` arrives and decides afterwards. It works under any
    /// `ProofStrategy`: under `App` or `None` it is the only proof the
    /// packet gets; under `All` it sends a second proof for a packet the
    /// link already proved on receipt, which the sender ignores, its
    /// `PacketProven` having consumed the recorded hash on the first match.
    /// The proof goes out on the link's bound interface, from an in-link or
    /// an out-link alike.
    ///
    /// Returns `Err(RnsError::InvalidArgument)` when no link of this
    /// transport has `link_id`, and `Err(RnsError::ConnectionError)` when
    /// the link is not yet or no longer `Active`, as `Transport::request`
    /// does (Python's `Packet.send` drops a packet on a `CLOSED` link,
    /// `RNS/Packet.py:294-299`), or when the proof cannot be sent, the link
    /// having no bound interface or the interface taking nothing.
    ///
    /// Fork-only convenience: a Python application calls `packet.prove()`
    /// on the packet its link callback received (`RNS/Packet.py:336-341`),
    /// which reaches `Link.prove_packet` (`RNS/Link.py:378-389`).
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn prove_link_packet(
        &self,
        link_id: &AddressHash,
        packet_hash: &Hash,
    ) -> Result<(), RnsError> {
        let Some(link) = self.find_any_link(link_id).await else {
            return Err(RnsError::InvalidArgument);
        };
        let proof = {
            let guard = link.lock().await;
            if guard.status() != LinkStatus::Active {
                return Err(RnsError::ConnectionError);
            }
            guard.prove_hash(packet_hash)
        };
        if self.send_link_packet_on_bound_iface(&link, proof).await.is_sent() {
            Ok(())
        } else {
            Err(RnsError::ConnectionError)
        }
    }
}
