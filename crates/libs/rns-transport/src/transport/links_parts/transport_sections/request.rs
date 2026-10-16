use crate::destination::link::LinkRequest;
use crate::destination::request::{PendingRequest, RequestId, RequestOptions, RequestReceipt};

/// What `Transport::request` decided under the link lock.
enum OutboundRequestForm {
    /// The request fits one `REQUEST` packet, built and registered already.
    Packet { id: RequestId, packet: Packet, receipt: RequestReceipt },
    /// The request goes out as a Resource; the registration waits until the
    /// Resource hash is known.
    Resource { timeout: Duration, iface: Option<AddressHash> },
}

impl Transport {
    /// Sends a request for `path` carrying `data` over the link `link_id`
    /// and returns the handle its response arrives on: `RNS.Link.request`
    /// (`RNS/Link.py:480-510`).
    ///
    /// The request packs as `Link::request_payload` does and goes out as
    /// one `REQUEST` packet when it fits `Link::link_mdu()`, otherwise as a
    /// request Resource (`RNS/Link.py:493-510`). The response is matched to
    /// the receipt by request id as it arrives on the link, whether as a
    /// packet or a Resource; the `LinkEvent::Data` and `resource_events()`
    /// broadcasts still carry it. `RnsError::InvalidArgument` names an
    /// unknown link, `RnsError::ConnectionError` a link that is not active
    /// or a packet that reached no interface.
    #[deny(clippy::await_holding_invalid_type)]
    pub async fn request(
        &self,
        link_id: &AddressHash,
        path: &str,
        data: rmpv::Value,
        opts: RequestOptions,
    ) -> Result<RequestReceipt, RnsError> {
        let link = self.find_any_link(link_id).await.ok_or(RnsError::InvalidArgument)?;
        let request = Link::request_payload(path, data)?;
        let form = {
            let mut guard = link.lock().await;
            if guard.status() != LinkStatus::Active {
                return Err(RnsError::ConnectionError);
            }
            let timeout = opts.timeout_for_rtt(guard.rtt());
            if request.packed.len() <= guard.link_mdu() {
                let packet = guard.request_packet_with_max_response_size(
                    &request.packed,
                    opts.max_response_size(),
                )?;
                let id = RequestId::from_packet_hash(&packet.hash());
                let receipt = guard.insert_pending_request(id, PendingRequest::new(id, timeout, None));
                OutboundRequestForm::Packet { id, packet, receipt }
            } else {
                OutboundRequestForm::Resource { timeout, iface: guard.ingress_iface() }
            }
        };
        match form {
            OutboundRequestForm::Packet { id, packet, receipt } => {
                let outcome = self.send_link_packet_on_bound_iface(&link, packet).await;
                if !outcome_sent(outcome) {
                    let mut guard = link.lock().await;
                    guard.remove_pending_request(&id);
                    guard.clear_response_size_limit(id.as_bytes());
                    return Err(RnsError::ConnectionError);
                }
                Ok(receipt)
            }
            OutboundRequestForm::Resource { timeout, iface } => {
                self.send_request_as_resource(
                    &link,
                    request,
                    timeout,
                    iface,
                    opts.max_response_size(),
                )
                .await
            }
        }
    }

    /// The resource branch of `RNS.Link.request` (`RNS/Link.py:503-510`).
    /// Follows `send_request_resource_with_max_response_size` step for step,
    /// except that the pending entry is registered, with the Resource hash,
    /// before the advertisement leaves: the sender-side outcome events carry
    /// only that hash.
    #[deny(clippy::await_holding_invalid_type)]
    async fn send_request_as_resource(
        &self,
        link: &Arc<Mutex<Link>>,
        request: LinkRequest,
        timeout: Duration,
        iface: Option<AddressHash>,
        max_response_size: Option<usize>,
    ) -> Result<RequestReceipt, RnsError> {
        let id = RequestId::new(request.resource_request_id);
        let interface_mtu = self.resource_mtu_for_iface(iface).await;
        // See `send_resource_observed`: prepared off the handler lock.
        let prepared = {
            let link_guard = link.lock().await;
            // See `resource_wire.rs`: the negotiated link MTU, not the local
            // interface alone, is what a fragment has to fit through.
            let interface_mtu = interface_mtu.min(link_guard.link_mtu());
            ResourceManager::prepare_send(
                &link_guard,
                request.packed,
                None,
                Some(id.to_bytes().to_vec()),
                false,
                interface_mtu,
            )?
        };
        let (resource_hash, packet) =
            self.handler.lock().await.resource_manager.track_prepared(prepared);
        let receipt = {
            let mut link_guard = link.lock().await;
            if let Some(max_response_size) = max_response_size {
                link_guard.set_response_size_limit(id.as_bytes(), max_response_size);
            }
            link_guard
                .insert_pending_request(id, PendingRequest::new(id, timeout, Some(resource_hash)))
        };
        let outcome = self.send_link_packet_on_bound_iface(link, packet).await;
        let sent = outcome_sent(outcome);
        if !sent {
            let mut link_guard = link.lock().await;
            link_guard.clear_response_size_limit(id.as_bytes());
            link_guard.remove_pending_request(&id);
        }
        let mut handler = self.handler.lock().await;
        handler.resource_manager.confirm_outbound_dispatch(resource_hash, sent);
        let events = handler.resource_manager.drain_events();
        super::resource_wire::publish_resource_events(&handler, events);
        if sent {
            Ok(receipt)
        } else {
            Err(RnsError::ConnectionError)
        }
    }
}

fn outcome_sent(outcome: SendPacketOutcome) -> bool {
    matches!(outcome, SendPacketOutcome::SentDirect | SendPacketOutcome::SentBroadcast)
}
