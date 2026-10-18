use super::path::send_to_next_hop;
use super::*;
use crate::destination::request::{
    DispatchForm, InboundResponseHashes, RequestError, RequestId, ResponseArrival,
};
use crate::hash::ADDRESS_HASH_SIZE;
use crate::resource::{build_link_packet, ResourceAdvertisement, ResourceEventKind};

pub(super) fn is_link_resource_packet(packet: &Packet) -> bool {
    packet.header.destination_type == DestinationType::Link
        && matches!(
            packet.context,
            PacketContext::Resource
                | PacketContext::ResourceAdvrtisement
                | PacketContext::ResourceRequest
                | PacketContext::ResourceHashUpdate
                | PacketContext::ResourceProof
                | PacketContext::ResourceInitiatorCancel
                | PacketContext::ResourceReceiverCancel
        )
}

pub(super) fn is_link_resource_proof(packet: &Packet) -> bool {
    packet.context == PacketContext::ResourceProof
        && packet.header.destination_type == DestinationType::Link
}

pub(super) async fn handle_link_resource_packet<'a>(
    packet: &Packet,
    iface: AddressHash,
    handler: &mut MutexGuard<'a, TransportHandler>,
) -> bool {
    let link = link_for_resource_packet(handler, packet).await;
    let Some(link) = link else {
        log::debug!(
            "[resource-diag] wire_resource_no_link node={} link={} ctx={:02x}",
            handler.config.name,
            packet.destination,
            packet.context as u8
        );
        return false;
    };

    let mut link = link.lock().await;
    // Resource advertisements, requests, hash updates and parts are routed
    // through the ResourceManager rather than `Link::handle_packet`. They
    // are still live link traffic and must prevent the active-link watchdog
    // from closing a long transfer.
    link.note_inbound(packet.context);
    log::debug!(
        "[resource-diag] wire_resource_packet node={} link={} ctx={:02x} has_ingress={}",
        handler.config.name,
        packet.destination,
        packet.context as u8,
        link.ingress_iface().is_some()
    );
    let packet_for_manager = match packet_for_resource_manager(packet, &mut link) {
        Ok(packet) => packet,
        Err(_) => {
            let (cancels, events) = owed_response_cancels(handler, &mut link);
            drop(link);
            return send_owed_response_cancels(handler, cancels, events).await;
        }
    };
    // A rejected advertisement never reaches the ResourceManager, and the
    // reject, like every receiver cancel, is only sent once the link guard
    // is dropped: `handler.send` re-locks the same link in
    // `note_link_packet_sent`, so sending it here would deadlock the whole
    // transport.
    let mut rejected = false;
    let mut cancels = Vec::new();
    let mut cancel_events = Vec::new();
    let mut response_advertised: Option<(Vec<u8>, ResourceAdvertisement)> = None;
    if packet.context == PacketContext::ResourceAdvrtisement {
        if let Ok(advertisement) = ResourceAdvertisement::unpack(packet_for_manager.data.as_slice())
        {
            if advertisement.is_response() {
                if let Some(request_id) = advertisement.request_id.as_ref() {
                    // `RNS/Link.py:1046-1049` measures `d` (`read_size`,
                    // `RNS/Resource.py:1285-1287`), the whole response's
                    // data size on every segment, against the request's
                    // `max_response_size`; a `q` no pending request matches
                    // falls out of the loop, neither accepted nor rejected.
                    let data_size = usize::try_from(advertisement.data_size).unwrap_or(usize::MAX);
                    let Some(admitted) = link.response_size_allowed(request_id, data_size) else {
                        log::debug!(
                            "[resource-diag] drop_unsolicited_response_advertisement node={} link={} hash={} request_id={}",
                            handler.config.name,
                            link.id(),
                            advertisement.hash,
                            hex::encode(request_id)
                        );
                        let (cancels, events) = owed_response_cancels(handler, &mut link);
                        drop(link);
                        return send_owed_response_cancels(handler, cancels, events).await;
                    };
                    let pending_id = <[u8; ADDRESS_HASH_SIZE]>::try_from(request_id.as_slice())
                        .ok()
                        .map(RequestId::new);
                    if pending_id.is_none() {
                        log::debug!(
                            "[resource-diag] response_advertisement_request_id_len node={} link={} hash={} len={}",
                            handler.config.name,
                            link.id(),
                            advertisement.hash,
                            request_id.len()
                        );
                    }
                    if admitted {
                        response_advertised = Some((request_id.to_vec(), advertisement.clone()));
                    } else {
                        log::warn!(
                            "[resource-diag] reject_response_advertisement link={} hash={} data_size={} transfer_size={} limit={:?}",
                            link.id(),
                            advertisement.hash,
                            advertisement.data_size,
                            advertisement.transfer_size,
                            link.response_size_limit(request_id)
                        );
                        // A `send_request_resource*` id of another length
                        // never becomes a `RequestId`, so its record is
                        // taken here. A receipt-owned id's record never holds
                        // the inbound hashes (the receipt does), so at most
                        // one of the two cancels fires for an id.
                        if let Some(receiving) = link.take_response_record(request_id) {
                            cancel_inbound_response(
                                &mut handler.resource_manager,
                                &link,
                                receiving,
                                &mut cancels,
                                &mut cancel_events,
                            );
                        }
                        if let Some(id) = pending_id {
                            if let Some(receiving) =
                                link.fail_pending_request(&id, RequestError::Oversize)
                            {
                                cancel_inbound_response(
                                    &mut handler.resource_manager,
                                    &link,
                                    receiving,
                                    &mut cancels,
                                    &mut cancel_events,
                                );
                            }
                        }
                        rejected = true;
                        cancels.extend(receiver_cancel(&link, Some(iface), &advertisement.hash));
                    }
                }
            }
            if !rejected && advertisement.is_request() {
                // `RNS/Link.py:1036-1046` measures the advertised data size
                // `d` (`ResourceAdvertisement.read_size`,
                // `RNS/Resource.py:1285-1287`) against `max_request_size`,
                // not the compressed transfer size `t`.
                let limit = handler
                    .single_in_destinations
                    .get(&link.destination().address_hash)
                    .cloned()
                    .map(|destination| async move { destination.lock().await.max_request_size() });
                if let Some(limit) = limit {
                    if let Some(limit) = limit.await {
                        if advertisement.data_size > u64::try_from(limit).unwrap_or(u64::MAX) {
                            log::warn!(
                                "[resource-diag] reject_request_advertisement link={} hash={} data_size={} transfer_size={} limit={}",
                                link.id(),
                                advertisement.hash,
                                advertisement.data_size,
                                advertisement.transfer_size,
                                limit
                            );
                            rejected = true;
                            cancels.extend(receiver_cancel(
                                &link,
                                Some(iface),
                                &advertisement.hash,
                            ));
                        }
                    }
                }
            }
        }
    }
    let response_iface = link.ingress_iface().unwrap_or(iface);
    // The smaller of what this node's interface can carry and what the link
    // actually negotiated. The interface alone is not enough: it describes
    // the first hop, while a resource fragment has to survive the whole
    // path, and the negotiated value is the only number that knows about
    // the far end.
    let interface_mtu = handler
        .iface_manager
        .lock()
        .await
        .mtu(&response_iface)
        .unwrap_or(crate::resource::DEFAULT_RESOURCE_INTERFACE_MTU)
        .min(link.link_mtu());
    let mut responses = std::mem::take(&mut handler.resource_response_packets);
    // A cancel the link owed fails the displaced assembly before this
    // packet's advertisement is admitted. Its `InboundFailed` comes back in
    // `cancel_events` and never enters the manager's queue, so a retry of
    // the same hash admitted by this packet keeps its fresh record. What
    // the manager queued for this link before this packet is routed below
    // only when the packet is admitted; a refused or unsolicited packet
    // leaves it for the periodic drain, and other links' events wait for
    // that periodic drain as well.
    drain_owed_response_cancels(
        &mut handler.resource_manager,
        &mut link,
        &mut cancels,
        &mut cancel_events,
    );
    let mut events = Vec::new();
    if !rejected {
        handler.resource_manager.handle_packet_into_with_mtu(
            &packet_for_manager,
            &mut link,
            &mut responses,
            interface_mtu,
        );
        events = handler.resource_manager.drain_events_for_link(link.id());
        // `RNS/Link.py:1055-1064` touches the receipt only once a Resource
        // exists for the advertisement: one the manager dropped (inbound
        // caps, an out-of-order or duplicate segment, an unreasonable part
        // count) leaves the receipt as it was, with its clock running.
        if let Some((id, advertisement)) = &response_advertised {
            if handler.resource_manager.has_active_receiver(&advertisement.hash, link.id(), id) {
                link.mark_pending_request_receiving(id, advertisement);
            }
        }
        // `RNS/Link.py:885-889`: an assembled request resource reaches the
        // handler table like a request packet, and an assembled response
        // resource resolves the receipt waiting for it (`RNS/Link.py:897-912`).
        // The events are still published below for `resource_events()`
        // consumers, except an assembled response the link refused.
        let mut suppressed = Vec::new();
        for (index, event) in events.iter().enumerate() {
            debug_assert_eq!(event.link_id, *link.id(), "the drain was for this link only");
            match &event.kind {
                ResourceEventKind::Complete(complete) if complete.is_request => {
                    link.dispatch_request(DispatchForm::Resource, &complete.data);
                }
                ResourceEventKind::Complete(complete) if complete.is_response => {
                    if let Some(request_id) = complete.request_id.as_deref() {
                        // The advertised `d` was the peer's claim, so the
                        // assembled size is measured against the record's
                        // limit once more, whatever the id's length. The
                        // Resource is already assembled and gone from the
                        // ResourceManager, so withholding the event is the
                        // failure; the arms below fail the receipt or clear
                        // the entry.
                        let over_limit = link
                            .response_size_limit(request_id)
                            .filter(|limit| complete.data.len() > *limit);
                        if let Some(limit) = over_limit {
                            log::warn!(
                                "link({}): response to {} assembled {} bytes over its {} byte limit",
                                link.id(),
                                hex::encode(request_id),
                                complete.data.len(),
                                limit
                            );
                            suppressed.push(index);
                        }
                        match <[u8; ADDRESS_HASH_SIZE]>::try_from(request_id) {
                            Ok(id) => {
                                let receiving = link.resolve_pending_request(
                                    RequestId::new(id),
                                    ResponseArrival::Resource {
                                        data: &complete.data,
                                        metadata: complete.metadata.as_deref(),
                                    },
                                );
                                // The Resource is assembled, so nothing is
                                // inbound for this id: this consumes the
                                // `#[must_use]` and cancels nothing.
                                if let Some(receiving) = receiving {
                                    cancel_inbound_response(
                                        &mut handler.resource_manager,
                                        &link,
                                        receiving,
                                        &mut cancels,
                                        &mut cancel_events,
                                    );
                                }
                            }
                            // A `send_request_resource*` id of another
                            // length has no receipt, only its entry, whose
                            // hashes name this assembled Resource: consumed
                            // the same way, cancelling nothing.
                            Err(_) => {
                                let (_, receiving) =
                                    link.take_response_limit(request_id, complete.data.len());
                                if let Some(receiving) = receiving {
                                    cancel_inbound_response(
                                        &mut handler.resource_manager,
                                        &link,
                                        receiving,
                                        &mut cancels,
                                        &mut cancel_events,
                                    );
                                }
                            }
                        }
                    }
                }
                ResourceEventKind::Progress(progress) => {
                    link.note_pending_response_progress(event.hash, progress);
                }
                // A receiver cancel for a request Resource surfaces here as
                // `OutboundRejected`, the failure of a response Resource as
                // `InboundFailed`.
                kind => {
                    if let Some(receiving) = link.note_request_resource_outcome(event.hash, kind) {
                        cancel_inbound_response(
                            &mut handler.resource_manager,
                            &link,
                            receiving,
                            &mut cancels,
                            &mut cancel_events,
                        );
                    }
                }
            }
        }
        for index in suppressed.into_iter().rev() {
            events.remove(index);
        }
    }
    // A cancel on either branch, or one the link owed, fails a transfer
    // whose event goes out with this packet's rather than waiting for the
    // next.
    events.extend(cancel_events);
    if !responses.is_empty() {
        log::debug!(
            "[resource-diag] wire_resource_responses node={} link={} ctx={:02x} responses={} iface={}",
            handler.config.name,
            packet.destination,
            packet.context as u8,
            responses.len(),
            response_iface
        );
    }
    drop(link);
    for cancel in cancels {
        handler.send(cancel).await;
    }
    for response in responses.drain(..) {
        handler
            .send(TxMessage { tx_type: TxMessageType::Direct(response_iface), packet: response })
            .await;
    }
    handler.resource_response_packets = responses;
    publish_resource_events(handler, events);
    true
}

include!("resource_wire_parts/cancel.rs");
include!("resource_wire_parts/proof.rs");

async fn link_for_resource_packet(
    handler: &TransportHandler,
    packet: &Packet,
) -> Option<Arc<Mutex<Link>>> {
    link_by_id(&handler.in_links, &handler.out_links, &packet.destination).await
}

type LinkMap = HashMap<AddressHash, Arc<Mutex<Link>>>;

/// `in_links` is keyed by link id but `out_links` by destination hash, so an
/// initiator-side link is only found by scanning `out_links` for the id after
/// the keyed lookups miss.
#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn link_by_id(
    in_links: &LinkMap,
    out_links: &LinkMap,
    link_id: &AddressHash,
) -> Option<Arc<Mutex<Link>>> {
    let mut link = in_links.get(link_id).cloned().or_else(|| out_links.get(link_id).cloned());
    if link.is_none() {
        for candidate in out_links.values() {
            if *candidate.lock().await.id() == *link_id {
                link = Some(candidate.clone());
                break;
            }
        }
    }
    link
}

fn packet_for_resource_manager(packet: &Packet, link: &mut Link) -> Result<Packet, RnsError> {
    let needs_decrypt = matches!(
        packet.context,
        PacketContext::ResourceAdvrtisement
            | PacketContext::ResourceRequest
            | PacketContext::ResourceHashUpdate
            | PacketContext::ResourceInitiatorCancel
            | PacketContext::ResourceReceiverCancel
    );
    if !needs_decrypt {
        return Ok(packet.clone());
    }

    let mut buffer = PacketDataBuffer::new();
    let plain_len = match link.decrypt(packet.data.as_slice(), buffer.accuire_buf_max()) {
        Ok(plain) => plain.len(),
        Err(err) => {
            log::debug!(
                "[resource-diag] wire_resource_decrypt_failed link={} ctx={:02x} err={:?}",
                packet.destination,
                packet.context as u8,
                err
            );
            log::warn!("failed to decrypt packet: {:?}", err);
            return Err(RnsError::CryptoError);
        }
    };
    buffer.resize(plain_len);
    let mut plain_packet = packet.clone();
    plain_packet.data = buffer;
    Ok(plain_packet)
}

pub(super) fn publish_resource_events(handler: &TransportHandler, events: Vec<ResourceEvent>) {
    for event in events {
        let hash = event.hash;
        let link_id = event.link_id;
        if handler.resource_events_tx.send(event).is_err() {
            log::trace!(
                "[resource-diag] event has no active subscribers resource={} link={}",
                hash,
                link_id
            );
        }
    }
}
