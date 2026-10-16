use super::path::send_to_next_hop;
use super::*;
use crate::destination::request::{DispatchForm, RequestError, RequestId, ResponseArrival};
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

pub(super) async fn handle_resource_proof(
    packet: Packet,
    handler: Arc<Mutex<TransportHandler>>,
    iface: AddressHash,
) {
    let mut handler = handler.lock().await;
    let link = link_for_resource_packet(&handler, &packet).await;
    if let Some(link) = link {
        let mut link = link.lock().await;
        // Resource proofs are handled here instead of through
        // `Link::handle_packet`; keep the link watchdog's inbound activity
        // anchor in sync with the wire traffic.
        link.note_inbound(packet.context);
        let mut responses = std::mem::take(&mut handler.resource_response_packets);
        handler.resource_manager.handle_packet_into(&packet, &mut link, &mut responses);
        let events = handler.resource_manager.drain_events();
        // A proof for a request Resource moves its receipt to `Delivered`
        // (`RNS/Link.py:1372`).
        for event in &events {
            if event.link_id == *link.id() {
                link.note_request_resource_outcome(event.hash, &event.kind);
            }
        }
        // A proof can hand back the next segment's advertisement. Like every
        // Link-destination packet it goes out on the link's attached
        // interface (`RNS/Transport.py:1452-1454`); `send_packet` would try
        // the path table, where a link id never resolves.
        let response_iface = link.ingress_iface().unwrap_or(iface);
        drop(link);
        for response in responses.drain(..) {
            handler
                .send(TxMessage {
                    tx_type: TxMessageType::Direct(response_iface),
                    packet: response,
                })
                .await;
        }
        handler.resource_response_packets = responses;
        publish_resource_events(&handler, events);
    } else {
        let reverse_packet = if handler.config.transport_enabled {
            handler.link_table.handle_reverse_link_packet(&packet, iface)
        } else {
            None
        };
        if let Some((packet, target_iface)) = reverse_packet {
            log::debug!(
                "[tp-diag] resource_proof_reverse_forward node={} link={} iface={}",
                handler.config.name,
                packet.destination,
                target_iface
            );
            handler.send(TxMessage { tx_type: TxMessageType::Direct(target_iface), packet }).await;
            return;
        }

        // A Link can carry a Resource in either direction. When the Link
        // initiator is receiving a reverse-direction Resource, its proof
        // arrives on the requester side and must continue toward the original
        // Link destination. The reverse-table branch above intentionally only
        // handles packets arriving from the responder side; mirror normal Link
        // data forwarding for the opposite direction.
        let lookup = handler.link_table.original_destination(&packet.destination);
        if lookup.is_some() {
            let sent = send_to_next_hop(&packet, &handler, lookup).await;
            log::debug!(
                "[tp-diag] resource_proof_forward node={} link={} sent={}",
                handler.config.name,
                packet.destination,
                sent
            );
        }
    }
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
        Err(_) => return true,
    };
    // A rejected advertisement never reaches the ResourceManager, and the
    // reject itself is only sent once the link guard is dropped:
    // `handler.send` re-locks the same link in `note_link_packet_sent`, so
    // sending it here would deadlock the whole transport.
    let mut rejected = false;
    let mut reject = None;
    if packet.context == PacketContext::ResourceAdvrtisement {
        if let Ok(advertisement) = ResourceAdvertisement::unpack(packet_for_manager.data.as_slice())
        {
            if advertisement.is_response() {
                if let Some(request_id) = advertisement.request_id.as_ref() {
                    let response_size =
                        usize::try_from(advertisement.transfer_size).unwrap_or(usize::MAX);
                    let admitted = link.take_response_limit_if_allowed(request_id, response_size);
                    let pending_id = <[u8; ADDRESS_HASH_SIZE]>::try_from(request_id.as_slice())
                        .ok()
                        .map(RequestId::new);
                    if admitted {
                        if let Some(id) = pending_id {
                            link.mark_pending_request_receiving(
                                &id,
                                advertisement.data_size,
                                advertisement.transfer_size,
                                advertisement.hash,
                            );
                        }
                    } else {
                        log::warn!(
                            "[resource-diag] reject_response_advertisement link={} hash={} size={}",
                            link.id(),
                            advertisement.hash,
                            advertisement.transfer_size
                        );
                        if let Some(id) = pending_id {
                            link.fail_pending_request(&id, RequestError::Oversize);
                        }
                        rejected = true;
                        reject = advertisement_reject(&link, iface, &advertisement.hash);
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
                            reject = advertisement_reject(&link, iface, &advertisement.hash);
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
    let mut events = Vec::new();
    if !rejected {
        handler.resource_manager.handle_packet_into_with_mtu(
            &packet_for_manager,
            &mut link,
            &mut responses,
            interface_mtu,
        );
        events = handler.resource_manager.drain_events();
        // `RNS/Link.py:885-889`: an assembled request resource reaches the
        // handler table like a request packet, and an assembled response
        // resource resolves the receipt waiting for it (`RNS/Link.py:896-912`).
        // The events are still published below for `resource_events()`
        // consumers.
        for event in &events {
            if event.link_id != *link.id() {
                continue;
            }
            match &event.kind {
                ResourceEventKind::Complete(complete) if complete.is_request => {
                    link.dispatch_request(DispatchForm::Resource, &complete.data);
                }
                ResourceEventKind::Complete(complete) if complete.is_response => {
                    let id = complete
                        .request_id
                        .as_deref()
                        .and_then(|bytes| <[u8; ADDRESS_HASH_SIZE]>::try_from(bytes).ok());
                    if let Some(id) = id {
                        link.resolve_pending_request(
                            RequestId::new(id),
                            ResponseArrival::Resource {
                                data: &complete.data,
                                metadata: complete.metadata.as_deref(),
                            },
                        );
                    }
                }
                ResourceEventKind::Progress(progress) => {
                    link.note_pending_response_progress(event.hash, progress);
                }
                // A receiver cancel for a request Resource surfaces here as
                // `OutboundRejected`.
                kind => link.note_request_resource_outcome(event.hash, kind),
            }
        }
    }
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
    if let Some(reject) = reject {
        handler.send(reject).await;
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

fn advertisement_reject(link: &Link, iface: AddressHash, hash: &Hash) -> Option<TxMessage> {
    let Ok(packet) = build_link_packet(
        link,
        PacketType::Data,
        PacketContext::ResourceReceiverCancel,
        hash.as_slice(),
    ) else {
        return None;
    };
    Some(TxMessage {
        tx_type: TxMessageType::Direct(link.ingress_iface().unwrap_or(iface)),
        packet,
    })
}

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
