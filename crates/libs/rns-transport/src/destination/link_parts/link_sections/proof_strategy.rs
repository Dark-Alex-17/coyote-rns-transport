// The per-link proof policy and the hash-keyed proof builder, `include!`d
// into `link_parts/link.rs` beside `new.rs` so that section stays below the
// repository's 500-line limit; a file boundary, not a privacy one.

impl Link {
    /// Sets whether this side proves a plain Data packet (`PacketContext::None`)
    /// it receives on the link. `All`, the default on every link, proves each
    /// one; `App` asks the callback from [`Self::set_proof_requested_callback`]
    /// per packet; `None` never proves. Channel frames are proved under every
    /// strategy, as Python does (`RNS/Link.py:1151`). Python has no per-link
    /// setting: a link proves by its destination's `proof_strategy`
    /// (`RNS/Link.py:961-968`).
    pub fn set_proof_strategy(&mut self, strategy: ProofStrategy) {
        self.proof_strategy = strategy;
    }

    /// The strategy [`Self::set_proof_strategy`] set, `All` until it is called.
    /// Fork-only: Python reads `self.destination.proof_strategy` (`RNS/Link.py:961`).
    pub fn proof_strategy(&self) -> ProofStrategy {
        self.proof_strategy
    }

    /// Registers the callback consulted per plain Data packet while the
    /// strategy is `App`. Without one an `App` link never proves by itself,
    /// which matters for an out-link: [`Link::new`] builds it without a
    /// destination, so nothing is copied onto it, where Python's initiator
    /// reads its OUT destination's callback (`RNS/Link.py:961-967`). A link a
    /// destination accepts starts with the destination's callback.
    ///
    /// Same contract as `ProofRequestedHandler`: the callback runs
    /// synchronously under the link lock and the transport handler lock, so it
    /// must not block and must not lock either of them.
    pub fn set_proof_requested_callback(&mut self, handler: Box<dyn ProofRequestedHandler>) {
        self.proof_requested_callback = Some(Arc::from(handler));
    }

    pub(crate) fn set_proof_policy(
        &mut self,
        strategy: ProofStrategy,
        callback: Option<Arc<dyn ProofRequestedHandler>>,
    ) {
        self.proof_strategy = strategy;
        self.proof_requested_callback = callback;
    }

    /// Mirror of the destination-level consult in `transport::wire::handle_data`.
    pub(crate) fn should_auto_prove(&self, packet: &Packet) -> bool {
        match self.proof_strategy {
            ProofStrategy::None => false,
            ProofStrategy::All => true,
            ProofStrategy::App => {
                self.proof_requested_callback.as_ref().is_some_and(|cb| cb.proof_requested(packet))
            }
        }
    }

    /// Builds the proof of the link packet with hash `hash`: the hash followed
    /// by this side's signature of it, as `Link.prove_packet` sends
    /// (`RNS/Link.py:378-389`) when an application calls `packet.prove()`
    /// (`RNS/Packet.py:336-341`). The hash is the one `LinkPayload::packet_hash`
    /// carries, so the packet itself need not be kept.
    pub fn prove_hash(&self, hash: &Hash) -> Packet {
        let hash = hash.to_bytes();
        let signature = self.priv_identity.sign(&hash).to_bytes();
        let mut packet_data = PacketDataBuffer::new();

        packet_data.safe_write(&hash);
        packet_data.safe_write(&signature);

        Packet {
            header: Header {
                packet_type: PacketType::Proof,
                destination_type: DestinationType::Link,
                ..Default::default()
            },
            ifac: None,
            destination: self.id,
            transport: None,
            // Python Reticulum emits ordinary per-packet Link proofs with
            // CONTEXT_NONE. LINKPROOF is accepted on receive for compatibility
            // with older LXMF-rs peers, but must not be our wire default.
            context: PacketContext::None,
            data: packet_data,
        }
    }
}
