// The Resource proof path of `resource_wire.rs`, `include!`d into that
// module so it stays below the repository's 500-line limit; a file boundary,
// not a privacy one.

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
        let mut events = handler.resource_manager.drain_events();
        // A proof for a request Resource moves its receipt to `Delivered`
        // (`RNS/Link.py:1372`).
        let mut cancels = Vec::new();
        for event in &events {
            if event.link_id == *link.id() {
                if let Some(receiving) = link.note_request_resource_outcome(event.hash, &event.kind)
                {
                    cancel_inbound_response(
                        &mut handler.resource_manager,
                        &link,
                        receiving,
                        &mut cancels,
                    );
                }
            }
        }
        drain_owed_response_cancels(&mut handler.resource_manager, &mut link, &mut cancels);
        events.extend(handler.resource_manager.drain_events());
        // A proof can hand back the next segment's advertisement. Like every
        // Link-destination packet it goes out on the link's attached
        // interface (`RNS/Transport.py:1452-1454`); `send_packet` would try
        // the path table, where a link id never resolves.
        let response_iface = link.ingress_iface().unwrap_or(iface);
        drop(link);
        for cancel in cancels {
            handler.send(cancel).await;
        }
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
