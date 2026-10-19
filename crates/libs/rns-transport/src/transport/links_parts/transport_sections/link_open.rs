// Kept separate from `close_channel.rs` so that section stays below the
// repository's 500-line limit. `include!`d into the same module.

impl Transport {
    /// Opens an outbound link to `destination`, reusing an existing open
    /// out-link when there is one.
    ///
    /// Fails with [`RnsError::NoPath`] when the path table has no next hop
    /// for the destination and this transport does not broadcast: the request
    /// would be dropped as `DroppedNoRoute` on the way out, and the caller
    /// would previously only learn that when the link timed out after
    /// `DEFAULT_ESTABLISHMENT_TIMEOUT` (12 s). With `broadcast` on the
    /// request is sent on every outgoing interface, as `RNS/Transport.py:1443`
    /// does for a destination without a path entry.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn link(&self, destination: DestinationDesc) -> Result<Arc<Mutex<Link>>, RnsError> {
        let link = self.handler.lock().await.out_links.get(&destination.address_hash).cloned();

        if let Some(link) = link {
            let status = link.lock().await.status();
            log::debug!(
                "[tp-diag] reuse_out_link destination={} status={:?}",
                destination.address_hash,
                status
            );
            if status != LinkStatus::Closed {
                return Ok(link);
            } else {
                log::warn!("tp({}): link was closed", self.name);
            }
        }

        let (next_hop_iface, hops, broadcast) = {
            let handler = self.handler.lock().await;
            (
                handler.path_table.next_hop_iface(&destination.address_hash),
                handler.path_table.hops_to(&destination.address_hash),
                handler.config.broadcast,
            )
        };
        if next_hop_iface.is_none() && !broadcast {
            log::debug!(
                "tp({}): no path to {} and broadcast is off, link request not sent",
                self.name,
                destination.address_hash
            );
            return Err(RnsError::NoPath);
        }

        let mut link = Link::new(destination, self.link_out_event_tx.clone());
        let next_hop_mtu = match next_hop_iface {
            Some(iface) => self.iface_manager.lock().await.mtu(&iface),
            None => None,
        };
        link.set_establishment_timeout(
            self.establishment_timeout_for(&destination.address_hash, next_hop_mtu, hops).await,
        );
        let packet = match next_hop_mtu {
            Some(mtu) => link.request_with_mtu(mtu),
            None => link.request(),
        };

        log::debug!(
            "tp({}): create new link {} for destination {}",
            self.name,
            link.id(),
            destination
        );
        log::debug!(
            "[tp-diag] create_out_link destination={} link_id={}",
            destination.address_hash,
            link.id()
        );

        let link = Arc::new(Mutex::new(link));

        self.handler.lock().await.out_links.insert(destination.address_hash, link.clone());

        self.send_packet(packet).await;

        Ok(link)
    }
}
