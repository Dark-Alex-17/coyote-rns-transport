// Loopback coverage for the receipt-less twin of `..._sizes_cancel.rs`: a record made by
// `request_packet_with_max_response_size` has no receipt to fail, so when an over-limit
// response packet refuses it while its response Resource is inbound, the record itself owes
// that Resource its receiver cancel (`RNS/Link.py:1435`, `RNS/Resource.py:1111-1116`) and
// the partial assembly does not go on to complete unmeasured. Helpers come from the sibling
// `tests_parts` files.

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_cancel2_oversize_packet_cancels_a_receipt_less_records_resource(
) {
    let pair = loopback_request_pair("sizes-cancel2-receipt-less-receiving").await;
    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id =
        sizes_probe2_send_limited_request_packet(&pair, "/quiet", Some(SIZES_TIGHT_LIMIT)).await;
    let advertisement = sizes_cancel_unbacked_response_advertisement(id, SIZES_CANCEL_ADMITTED_D);
    inject_advertisement_from_listener(&pair, &advertisement).await;
    // The connector asks for parts only once the record is marked receiving.
    await_link_packets(&mut listener_tap, link_id, PacketContext::ResourceRequest, 1).await;

    let envelope = sizes_cancel_probe_oversize_envelope(id);
    sizes_cancel_send_response_packet_from_listener(&pair, &envelope).await;

    let failed = await_resource_event(&mut events, |event| {
        event.link_id == link_id
            && match &event.kind {
                ResourceEventKind::InboundFailed(_) => true,
                ResourceEventKind::Complete(complete) => {
                    panic!("the refused record's response was assembled: {:?}", complete.request_id)
                }
                _ => false,
            }
    })
    .await;
    match &failed.kind {
        ResourceEventKind::InboundFailed(failure) => {
            assert_eq!(failure.reason, "cancelled", "a local cancel, not a retry exhaustion")
        }
        kind => panic!("not an inbound failure: {kind:?}"),
    }
    assert_eq!(
        failed.hash, advertisement.original_hash,
        "the transfer the record was receiving is dropped (a single-segment advertisement: its original hash is its own)"
    );

    sizes_assert_link_still_answers(&pair).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        1,
        "the response Resource the refused record was receiving is cancelled to the responder"
    );
}
