impl AutoRuntimePlan {
    pub fn discovery_state(&self) -> AutoDiscoveryState {
        AutoDiscoveryState::from_timing(
            self.adopted_devices.clone(),
            AutoInterfaceTiming::for_platform(self.platform),
        )
    }

    pub fn process_discovery_datagram(
        &self,
        state: &mut AutoDiscoveryState,
        datagram: AutoDiscoveryDatagram,
        now: core::time::Duration,
    ) -> Result<Option<AutoProcessedDiscoveryDatagram>, AutoDiscoveryRejectReason> {
        if now < self.startup_plan.initial_peering_wait {
            return Ok(None);
        }

        let source_address = discovery_source_address(&datagram);
        let event = state.observe_authenticated_discovery_packet(
            &datagram.payload,
            self.config.group_id.as_bytes(),
            &source_address,
            &datagram.ifname,
            now,
        )?;
        Ok(Some(AutoProcessedDiscoveryDatagram { datagram, source_address, event }))
    }

    pub fn process_peer_data_datagram(
        &self,
        state: &mut AutoDiscoveryState,
        dedupe: &mut AutoInboundPacketDeduplicator,
        datagram: AutoPeerDataDatagram,
        now: core::time::Duration,
    ) -> Option<AutoProcessedPeerDataDatagram> {
        if now < self.startup_plan.initial_peering_wait {
            return None;
        }

        let peer_address = peer_data_source_address(&datagram);
        let decision =
            state.handle_spawned_peer_inbound(dedupe, &peer_address, &datagram.payload, now);
        Some(AutoProcessedPeerDataDatagram { datagram, peer_address, decision })
    }
}
