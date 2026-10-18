use crate::destination::link::LinkRequest;
use crate::destination::request::{
    InboundResponseHashes, PendingRequest, RequestId, RequestOptions, RequestReceipt,
};

/// What `Transport::request` decided under the link lock.
enum OutboundRequestForm {
    /// The request fits one `REQUEST` packet, built and registered already.
    /// `displaced` holds the response Resources the record and the receipt
    /// entry under the same id were receiving, one each at most.
    Packet {
        id: RequestId,
        packet: Box<Packet>,
        receipt: RequestReceipt,
        displaced: Vec<InboundResponseHashes>,
    },
    /// The request goes out as a Resource; the registration waits until the
    /// Resource hash is known.
    Resource { timeout: Duration, iface: Option<AddressHash> },
}

impl Transport {
    /// Sends a request for `path` carrying `data` over the link `link_id`
    /// and returns the handle its response arrives on: `RNS.Link.request`
    /// (`RNS/Link.py:473-510`).
    ///
    /// The request packs as `Link::request_payload` does and goes out as
    /// one `REQUEST` packet when it fits `Link::link_mdu()`, otherwise as a
    /// request Resource (`RNS/Link.py:493-510`). The response is matched to
    /// the receipt by request id as it arrives on the link, whether as a
    /// packet or a Resource; the `LinkEvent::Data` and `resource_events()`
    /// broadcasts still carry it. `RnsError::InvalidArgument` names an
    /// unknown link, `RnsError::ConnectionError` a link that is not active
    /// or a packet that reached no interface. A `Stale` link is refused too,
    /// where Python drops a packet only on a `CLOSED` one
    /// (`RNS/Packet.py:294-295`).
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
                let packet = guard.request_packet(&request.packed)?;
                let id = RequestId::from_packet_hash(&packet.hash());
                let displaced_record =
                    guard.replace_response_record(id.as_bytes(), opts.max_response_size());
                let (receipt, displaced) =
                    guard.insert_pending_request(id, PendingRequest::new(id, timeout, None));
                OutboundRequestForm::Packet {
                    id,
                    packet: Box::new(packet),
                    receipt,
                    displaced: displaced_record.into_iter().chain(displaced).collect(),
                }
            } else {
                OutboundRequestForm::Resource { timeout, iface: guard.ingress_iface() }
            }
        };
        match form {
            OutboundRequestForm::Packet { id, packet, receipt, displaced } => {
                self.cancel_inbound_responses(&link, displaced).await;
                let outcome = self.send_link_packet_on_bound_iface(&link, *packet).await;
                if !outcome.is_sent() {
                    link.lock().await.remove_pending_request(&id);
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
        // The link was active when the form was chosen, under an earlier
        // guard; it may have closed in between.
        let registered = {
            let mut link_guard = link.lock().await;
            if link_guard.status() == LinkStatus::Active {
                let displaced_record =
                    link_guard.replace_response_record(id.as_bytes(), max_response_size);
                let (receipt, displaced) = link_guard.insert_pending_request(
                    id,
                    PendingRequest::new(id, timeout, Some(resource_hash)),
                );
                let displaced: Vec<_> = displaced_record.into_iter().chain(displaced).collect();
                Some((receipt, displaced))
            } else {
                None
            }
        };
        let sent = match &registered {
            Some((_, displaced)) => {
                self.cancel_inbound_responses(link, displaced.clone()).await;
                self.send_link_packet_on_bound_iface(link, packet).await.is_sent()
            }
            None => false,
        };
        if !sent && registered.is_some() {
            link.lock().await.remove_pending_request(&id);
        }
        let mut handler = self.handler.lock().await;
        handler.resource_manager.confirm_outbound_dispatch(resource_hash, sent);
        let events = handler.resource_manager.drain_events();
        super::resource_wire::publish_resource_events(&handler, events);
        match registered {
            Some((receipt, _)) if sent => Ok(receipt),
            _ => Err(RnsError::ConnectionError),
        }
    }

    /// Cancels the response Resources `receiving` that receipts failed, or
    /// records displaced, under a link guard were still receiving
    /// (`RNS/Link.py:1435`): the transfers are dropped and, while the link
    /// is active, the peer is sent a receiver cancel for each segment that
    /// was in flight (`RNS/Resource.py:1111-1116`). The handler and the
    /// link are locked one after the other, never together.
    #[deny(clippy::await_holding_invalid_type)]
    async fn cancel_inbound_responses(
        &self,
        link: &Arc<Mutex<Link>>,
        receiving: Vec<InboundResponseHashes>,
    ) {
        if receiving.is_empty() {
            return;
        }
        let link_id = *link.lock().await.id();
        let cancelled: Vec<Hash> = {
            let mut handler = self.handler.lock().await;
            let mut events = Vec::new();
            let cancelled = receiving
                .into_iter()
                .filter(|response| {
                    let cancelled = handler.resource_manager.cancel_incoming_response(
                        response.segment,
                        response.original,
                        &link_id,
                    );
                    events.extend(cancelled.event);
                    cancelled.receiver_in_flight
                })
                .map(|response| response.segment)
                .collect();
            super::resource_wire::publish_resource_events(&handler, events);
            cancelled
        };
        let packets: Vec<Packet> = {
            let link_guard = link.lock().await;
            if link_guard.status() == LinkStatus::Active {
                cancelled
                    .iter()
                    .filter_map(|hash| {
                        super::resource_wire::receiver_cancel_packet(&link_guard, hash)
                    })
                    .collect()
            } else {
                Vec::new()
            }
        };
        for packet in packets {
            self.send_link_packet_on_bound_iface(link, packet).await;
        }
    }
}
