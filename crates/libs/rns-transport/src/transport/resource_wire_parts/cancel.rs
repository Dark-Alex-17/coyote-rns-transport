// The receiver-cancel helpers of `resource_wire.rs`, `include!`d into that
// module so it stays below the repository's 500-line limit; a file boundary,
// not a privacy one.

/// A `RESOURCE_RCL` for `hash`: the receiver's "stop sending"
/// (`RNS/Resource.py:1111-1116`).
pub(super) fn receiver_cancel_packet(link: &Link, hash: &Hash) -> Option<Packet> {
    build_link_packet(
        link,
        PacketType::Data,
        PacketContext::ResourceReceiverCancel,
        hash.as_slice(),
    )
    .ok()
}

/// `receiver_cancel_packet` addressed to the link's bound interface, or to
/// `iface` while the link has none.
fn receiver_cancel(link: &Link, iface: Option<AddressHash>, hash: &Hash) -> Option<TxMessage> {
    let iface = link.ingress_iface().or(iface)?;
    let packet = receiver_cancel_packet(link, hash)?;
    Some(TxMessage { tx_type: TxMessageType::Direct(iface), packet })
}

/// A receipt failed while its response Resource was still inbound: the
/// transfer is dropped, whether a segment is in flight or only the partial
/// assembly of a split response remains, and while a segment was in flight
/// on an active link the peer is told to stop (`RNS/Link.py:1435`,
/// `RNS/Resource.py:1111-1116`). The cancel is queued on `cancels` for the
/// caller to send once the link guard is gone.
pub(super) fn cancel_inbound_response(
    resources: &mut ResourceManager,
    link: &Link,
    receiving: InboundResponseHashes,
    cancels: &mut Vec<TxMessage>,
) {
    if resources.cancel_incoming_response(receiving.segment, receiving.original, link.id())
        && link.status() == LinkStatus::Active
    {
        cancels.extend(receiver_cancel(link, None, &receiving.segment));
    }
}

/// Routes Resource outcomes drained by a periodic worker to the pending
/// requests of the links they belong to: a request Resource whose retries
/// ran out fails its receipt `Send`, a response Resource whose receiver gave
/// up fails its receipt `Receive`. The packet paths above route their own
/// events; this is for outcomes no packet carried. The cancels go out once
/// every link guard is gone.
#[deny(clippy::await_holding_invalid_type)]
pub(super) async fn note_resource_events_on_links(
    handler: &mut TransportHandler,
    events: &[ResourceEvent],
) {
    let mut cancels = Vec::new();
    for event in events {
        let link = link_by_id(&handler.in_links, &handler.out_links, &event.link_id).await;
        let Some(link) = link else {
            continue;
        };
        let mut link = link.lock().await;
        if let Some(receiving) = link.note_request_resource_outcome(event.hash, &event.kind) {
            cancel_inbound_response(&mut handler.resource_manager, &link, receiving, &mut cancels);
        }
    }
    for cancel in cancels {
        handler.send(cancel).await;
    }
}
