use super::diag;
use super::diag::bytes_to_hex;
use super::wire_encryption::should_encrypt_packet;
use super::*;
use crate::destination::request::RequestId;
use crate::iface::InterfaceMode;

impl TransportHandler {
    async fn note_link_packet_sent(&self, packet: &Packet) {
        let link = if packet.header.packet_type == PacketType::LinkRequest {
            let requested_id = crate::destination::link::LinkId::from(packet);
            let mut matched = None;
            for candidate in self.out_links.values() {
                if *candidate.lock().await.id() == requested_id {
                    matched = Some(candidate.clone());
                    break;
                }
            }
            matched
        } else if packet.header.destination_type == DestinationType::Link {
            if let Some(link) = self.in_links.get(&packet.destination).cloned() {
                Some(link)
            } else {
                let mut matched = None;
                for candidate in self.out_links.values() {
                    if *candidate.lock().await.id() == packet.destination {
                        matched = Some(candidate.clone());
                        break;
                    }
                }
                matched
            }
        } else {
            None
        };

        if let Some(link) = link {
            let mut link = link.lock().await;
            link.note_outbound(packet.context);
            // Every request packet solicits its response (RNS/Link.py:473-510,
            // :1046-1047); see Link::note_request_sent.
            if packet.context == PacketContext::Request {
                let request_id = RequestId::from_packet_hash(&packet.hash());
                link.note_request_sent(request_id.as_bytes());
            }
            // Every plain Data packet sent on a link is recorded for its proof
            // (RNS/Transport.py:1359-1377 records the receipt, narrowed here to
            // PacketContext::None; :2748-2761 consumes it); see Link::note_data_sent.
            if packet.header.packet_type == PacketType::Data
                && packet.context == PacketContext::None
            {
                link.note_data_sent(packet.hash());
            }
        }
    }

    async fn prepare_outbound_packet(
        &mut self,
        packet: &mut Packet,
    ) -> Result<(), SendPacketTrace> {
        if packet.header.packet_type == PacketType::Proof {
            log::trace!(
                "[tp] send_proof dst={} ctx={:02x}",
                packet.destination,
                packet.context as u8
            );
            if packet.context == PacketContext::LinkRequestProof {
                if let Ok(raw) = packet.to_bytes() {
                    log::trace!("[tp] lrproof_raw len={} hex={}", raw.len(), bytes_to_hex(&raw));
                }
            }
        }
        if should_encrypt_packet(packet) {
            let destination = self.single_out_destinations.get(&packet.destination).cloned();
            let Some(destination) = destination else {
                log::warn!(
                    "tp({}): missing destination identity for {}",
                    self.config.name,
                    packet.destination
                );
                return Err(SendPacketTrace::without_packet_hash(
                    SendPacketOutcome::DroppedMissingDestinationIdentity,
                ));
            };
            let identity = destination.lock().await.identity;
            let salt = identity.address_hash.as_slice();
            let ratchet =
                self.ratchet_store.as_mut().and_then(|store| store.get(&packet.destination));
            let public_key = ratchet.map(PublicKey::from).unwrap_or(identity.public_key);
            match encrypt_for_public_key(&public_key, salt, packet.data.as_slice(), OsRng) {
                Ok(ciphertext) => {
                    let mut buffer = PacketDataBuffer::new();
                    if buffer.write(&ciphertext).is_err() {
                        log::warn!(
                            "tp({}): ciphertext too large for packet to {}",
                            self.config.name,
                            packet.destination
                        );
                        return Err(SendPacketTrace::without_packet_hash(
                            SendPacketOutcome::DroppedCiphertextTooLarge,
                        ));
                    }
                    packet.data = buffer;
                }
                Err(err) => {
                    log::warn!(
                        "tp({}): encrypt failed for {}: {:?}",
                        self.config.name,
                        packet.destination,
                        err
                    );
                    return Err(SendPacketTrace::without_packet_hash(
                        SendPacketOutcome::DroppedEncryptFailed,
                    ));
                }
            }
        }

        Ok(())
    }

    pub(super) async fn send_packet(&mut self, packet: Packet) {
        let _ = self.send_packet_with_trace(packet).await;
    }

    pub(super) async fn send_packet_with_outcome(&mut self, packet: Packet) -> SendPacketOutcome {
        self.send_packet_with_trace(packet).await.outcome
    }

    pub(super) async fn send_packet_with_trace(&mut self, packet: Packet) -> SendPacketTrace {
        self.send_packet_observed_with_trace(packet, |_| {}).await
    }

    pub(super) async fn send_packet_observed_with_trace(
        &mut self,
        mut packet: Packet,
        observe_packet_hash: impl FnOnce(Hash),
    ) -> SendPacketTrace {
        if let Err(trace) = self.prepare_outbound_packet(&mut packet).await {
            return trace;
        }

        let packet_hash = packet.hash();
        observe_packet_hash(packet_hash);

        diag::log_route_lookup(&self.path_table, &packet.destination);

        let route = super::path::route_outbound_packet(
            &self.path_table,
            &packet,
            self.config.connected_to_shared_instance,
        );
        let packet = route.packet;
        if let Some(iface) = route.next_iface {
            let dispatch =
                self.send(TxMessage { tx_type: TxMessageType::Direct(iface), packet }).await;
            let outcome = if dispatch.sent_ifaces > 0 {
                SendPacketOutcome::SentDirect
            } else {
                SendPacketOutcome::DroppedNoRoute
            };
            diag::log_direct_send(iface, outcome, &dispatch);
            SendPacketTrace::with_packet_hash(outcome, packet_hash, Some(iface), false, dispatch)
        } else if self.config.broadcast || packet.header.packet_type == PacketType::Announce {
            let dispatch =
                self.send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet }).await;
            let outcome = if dispatch.sent_ifaces > 0 || dispatch.queued_ifaces > 0 {
                SendPacketOutcome::SentBroadcast
            } else {
                SendPacketOutcome::DroppedNoRoute
            };
            diag::log_broadcast_send(outcome, &dispatch);
            SendPacketTrace::with_packet_hash(outcome, packet_hash, None, true, dispatch)
        } else {
            log::trace!(
                "tp({}): no route for outbound packet dst={}",
                self.config.name,
                packet.destination
            );
            SendPacketTrace::with_packet_hash(
                SendPacketOutcome::DroppedNoRoute,
                packet_hash,
                None,
                false,
                TxDispatchTrace::default(),
            )
        }
    }

    pub(super) async fn send_packet_broadcast_with_trace(
        &mut self,
        mut packet: Packet,
    ) -> SendPacketTrace {
        if let Err(trace) = self.prepare_outbound_packet(&mut packet).await {
            return trace;
        }

        let packet_hash = packet.hash();
        let dispatch =
            self.send(TxMessage { tx_type: TxMessageType::Broadcast(None), packet }).await;
        let outcome = if dispatch.sent_ifaces > 0 || dispatch.queued_ifaces > 0 {
            SendPacketOutcome::SentBroadcast
        } else {
            SendPacketOutcome::DroppedNoRoute
        };
        SendPacketTrace::with_packet_hash(outcome, packet_hash, None, true, dispatch)
    }

    #[allow(clippy::await_holding_invalid_type)] // TASK-026: iface_manager guard spans the send under the caller's handler guard, see TransportHandler lock discipline
    pub(super) async fn send(&self, message: TxMessage) -> TxDispatchTrace {
        let packet = message.packet.clone();
        // Unguarded on purpose: receipt guard, see filter_duplicate_packet / PacketCache::update.
        self.packet_cache.lock().await.update(&packet);
        let announce_policy = if packet.header.packet_type == PacketType::Announce
            && matches!(message.tx_type, TxMessageType::Broadcast(_))
        {
            let next_hop_iface = self.path_table.next_hop_iface(&packet.destination);
            let mut mgr = self.iface_manager.lock().await;
            let local_destination = self.single_in_destinations.contains_key(&packet.destination);
            let policy =
                AnnounceBroadcastPolicy::for_next_hop(&mgr, next_hop_iface, local_destination);
            let dispatch = mgr.send_with_announce_policy(message, Some(policy)).await;
            if dispatch.sent_ifaces > 0 || dispatch.queued_ifaces > 0 {
                self.note_link_packet_sent(&packet).await;
            }
            return dispatch;
        } else {
            None
        };
        let dispatch = self
            .iface_manager
            .lock()
            .await
            .send_with_announce_policy(message, announce_policy)
            .await;
        if dispatch.sent_ifaces > 0 || dispatch.queued_ifaces > 0 {
            self.note_link_packet_sent(&packet).await;
        }
        dispatch
    }

    pub(super) fn has_destination(&self, address: &AddressHash) -> bool {
        self.single_in_destinations.contains_key(address)
    }

    pub(super) fn knows_destination(&self, address: &AddressHash) -> bool {
        self.single_out_destinations.contains_key(address)
    }

    #[cfg(test)]
    pub(super) async fn filter_duplicate_packets(&self, packet: &Packet) -> bool {
        let in_link = self.in_links.get(&packet.destination).cloned();
        // No arriving interface here, so the posture alone decides the skip.
        super::inbound_processing::filter_duplicate_packet(
            self.packet_cache.clone(),
            in_link,
            &self.config.name,
            packet,
            self.config.connected_to_shared_instance,
            *self.config.identity.address_hash(),
        )
        .await
        .0
    }

    pub(super) async fn request_path(
        &mut self,
        address: &AddressHash,
        on_iface: Option<AddressHash>,
        tag: Option<TagBytes>,
    ) {
        let packet = self.path_requests.generate(address, tag);

        let trace = self
            .send_recursive_path_request_with_modes(
                TxMessage { tx_type: TxMessageType::Broadcast(on_iface), packet },
                None,
            )
            .await;
        if trace.sent_ifaces > 0 || trace.queued_ifaces > 0 {
            self.path_requests.record_outgoing_request(address);
        }
    }

    /// Register (or refresh) the *virtual* unicast iface that the
    /// transport uses to route point-to-point traffic for the peer
    /// that delivered this packet. Only acts when:
    ///   - the packet arrived on a multicast iface, and
    ///   - that multicast iface has a registered `PeerRouting` map
    ///     (i.e. it was registered via
    ///     `Transport::add_multicast_udp_interface`), and
    ///   - the source is a UDP socket address.
    ///
    /// Returns the virtual iface hash to stick in the path_table so
    /// subsequent `Direct` tx for this peer's destinations is routed
    /// through the host multicast socket as a unicast send — and,
    /// symmetrically, so inbound replies from this peer (which arrive
    /// on the host multicast socket) get re-attributed to this same
    /// virtual iface by the host's rx task. That symmetry is what
    /// makes `Link::iface_matches` succeed on the proof/keepalive.
    pub(super) async fn unicast_iface_for_source(
        &mut self,
        rx_iface: AddressHash,
        source: IfaceSource,
    ) -> Option<AddressHash> {
        let peer = match source {
            IfaceSource::Udp(addr) => addr,
            IfaceSource::None => return None,
        };

        let role = { self.iface_manager.lock().await.role(&rx_iface) };
        if role != Some(IfaceRole::Multicast) {
            return None;
        }

        let peer_routing = self.multicast_peer_routings.get(&rx_iface).cloned()?;

        let now = Instant::now();
        if let Some(entry) = self.unicast_udp_ifaces.get_mut(&peer) {
            entry.1 = now;
            return Some(entry.0);
        }

        let virtual_hash = {
            let mut mgr = self.iface_manager.lock().await;
            mgr.register_virtual_iface(rx_iface, IfaceRole::VirtualUnicast)?
        };
        peer_routing.lock().await.insert(peer, virtual_hash);
        log::debug!(
            "tp({}): registered virtual UDP iface {} for peer {} on host {}",
            self.config.name,
            virtual_hash,
            peer,
            rx_iface,
        );
        self.unicast_udp_ifaces.insert(peer, (virtual_hash, now));
        Some(virtual_hash)
    }

    /// Whether an inbound link-associated packet targets something we will
    /// actually route or deliver — i.e. it is "accepted" rather than dropped.
    /// Used to gate eager unicast-route learning so that unauthenticated
    /// multicast senders cannot grow `unicast_udp_ifaces` / the `iface_manager`
    /// virtual-iface set with requests to unknown destinations.
    ///
    /// The criteria mirror the accept branches of the individual handlers:
    ///   - Link packets (`Proof` / `Data` with `DestinationType::Link`) are
    ///     accepted when we already track the link — as an inbound link, an
    ///     outbound link (matched by id), or a forward in the `link_table`.
    ///   - Everything else (`LinkRequest` and single `Data`, both
    ///     `DestinationType::Single`) is accepted when the destination is a
    ///     local input destination or, with transport enabled, a known next hop.
    pub(super) async fn should_learn_unicast_route(&self, packet: &Packet) -> bool {
        if packet.header.destination_type == DestinationType::Link {
            // `LinkId == AddressHash`, so the link id is `packet.destination`.
            if self.in_links.contains_key(&packet.destination)
                || self.link_table.knows(&packet.destination)
            {
                return true;
            }
            for link in self.out_links.values() {
                if *link.lock().await.id() == packet.destination {
                    return true;
                }
            }
            false
        } else {
            self.single_in_destinations.contains_key(&packet.destination)
                || self.config.transport_enabled
                    && self.path_table.next_hop_full(&packet.destination).is_some()
        }
    }

    /// Resolve the interface an inbound *link-associated* packet (LinkRequest /
    /// Proof / link Data) should be handled on, eagerly learning the sender's
    /// unicast route in the process.
    pub(super) async fn ingress_route_iface(
        &mut self,
        packet: &Packet,
        iface: AddressHash,
        source: IfaceSource,
    ) -> AddressHash {
        if !self.should_learn_unicast_route(packet).await {
            log::info!(
                "tp({}): unrecognized target: {}, keeping ingress as-is.",
                self.config.name,
                packet.destination.to_hex_string()
            );
            return iface;
        }
        match self.unicast_iface_for_source(iface, source).await {
            Some(virtual_iface) => virtual_iface,
            None => {
                log::trace!(
                    "tp({}): no unicast route learned for source {:?} on iface {}; keeping ingress iface",
                    self.config.name,
                    source,
                    iface,
                );
                iface
            }
        }
    }

    /// Register a `PeerRouting` map for a multicast iface at
    /// construction time. Called by
    /// `Transport::add_multicast_udp_interface`.
    pub(super) fn register_multicast_peer_routing(
        &mut self,
        iface: AddressHash,
        routing: Arc<Mutex<crate::iface::udp::PeerRouting>>,
    ) {
        self.multicast_peer_routings.insert(iface, routing);
    }

    pub(super) fn local_hops_delta_for_packet(&self, packet: &Packet) -> Option<u8> {
        let local_hops_delta = self.config.local_hops_delta;
        if self.config.connected_to_shared_instance
            || packet.header.hops != 0
            || local_hops_delta == 0
            || matches!(
                packet.header.destination_type,
                DestinationType::Plain | DestinationType::Group
            )
        {
            None
        } else {
            Some(local_hops_delta)
        }
    }
}

include!("handler_parts/path_request.rs");
include!("handler_parts/unicast_gc.rs");
