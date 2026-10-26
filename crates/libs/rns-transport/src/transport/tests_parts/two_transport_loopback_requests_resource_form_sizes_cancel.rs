// Fixtures for a receipt resolved while its response Resource is inbound: the response size
// limit is read, not spent, when a response Resource is admitted (`RNS/Link.py:1046-1049`),
// so a response packet arriving while the receipt is `Receiving` is still measured against
// it (`RNS/Link.py:874-876`), and the response Resource it leaves behind is cancelled with a
// receiver cancel to the responder (`RNS/Link.py:1435`, `RNS/Resource.py:1111-1116`). The
// tests live in `two_transport_loopback_requests_resource_form_sizes_cancel_probe.rs`.

/// `d` of an unbacked response advertisement the tight limit admits.
const SIZES_CANCEL_ADMITTED_D: u64 = (SIZES_TIGHT_LIMIT / 2) as u64;

/// `d` of an unbacked response advertisement the tight limit refuses.
const SIZES_OVER_D: u64 = (SIZES_TIGHT_LIMIT + 64) as u64;

/// A single-part response advertisement for `id` the ResourceManager admits, with `d` and
/// `t` of `data_size`. Nothing backs it on the responder, so the receiver it builds stays
/// inbound, waiting on its part request, until something cancels it.
fn sizes_cancel_unbacked_response_advertisement(
    id: RequestId,
    data_size: u64,
) -> ResourceAdvertisement {
    let hash = crate::hash::Hash::new_from_rand(OsRng);
    ResourceAdvertisement {
        transfer_size: data_size,
        data_size,
        parts: 1,
        hash,
        random_hash: [0; crate::resource::RANDOM_HASH_SIZE],
        original_hash: hash,
        segment_index: 1,
        total_segments: 1,
        request_id: Some(serde_bytes::ByteBuf::from(id.to_bytes().to_vec())),
        flags: crate::resource::FLAG_RESPONSE,
        hashmap: Vec::new(),
    }
}

/// Sends `envelope` from the responder over the pair's link as a response packet.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_cancel_send_response_packet_from_listener(
    pair: &LoopbackRequestPair,
    envelope: &[u8],
) {
    let in_link = pair
        .handles
        .listener
        .transport
        .find_in_link(&pair.handles.link_id)
        .await
        .expect("the listener tracks the inbound link");
    let packet =
        in_link.lock().await.response_packet(envelope).expect("the response fits a packet");
    let outcome =
        pair.handles.listener.transport.send_link_packet_on_bound_iface(&in_link, packet).await;
    assert_eq!(outcome, SendPacketOutcome::SentDirect, "the response packet reaches the wire");
}
