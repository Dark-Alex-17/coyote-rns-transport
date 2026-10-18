// Usage-pattern coverage of the contract pinned by
// `two_transport_loopback_request_receipt_failed_response_resource_fails_receive`, with the
// race biased: a response Resource that fails in transfer fails the receipt `Receive`
// rather than leaving it `Receiving` with no clock (`RNS/Link.py:1389`, `:1435`). That
// fixture races the retry worker's periodic drain against the link's next inbound Resource
// packet; this one makes the inbound packet all but certain to win. The failure is produced
// by dropping the connector's Resource state for the link, as the receiver giving up or a
// link close does, and the next packet on the link is a Resource packet: an advertisement
// under an id nothing solicited, which the link drops. The failure already queued must still
// reach the receipt, whichever drain finds it first. Helpers come from the sibling
// `tests_parts` files.

/// A receipt `Receiving` an unbacked response has its inbound Resource dropped, then an
/// unsolicited advertisement reaches the link before the retry worker's next drain: the
/// receipt fails `Receive` within the loopback budget all the same, and the link still
/// answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe13_a_queued_inbound_failure_reaches_the_receipt_past_an_unrelated_resource_packet(
) {
    let pair = loopback_request_pair("sizes-probe13-queued-failure-past-resource-packet").await;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;

    let (receipt, _status_rx) = sizes_cancel_probe_receiving_receipt(&pair).await;
    drop_resource_state(&pair.handles.connector.transport, pair.handles.link_id).await;

    // The next inbound packet on the link is a Resource packet nothing solicited.
    let unsolicited = sizes_cancel_unbacked_response_advertisement(
        RequestId::new([0x13; ADDRESS_HASH_SIZE]),
        SIZES_CANCEL_ADMITTED_D,
    );
    inject_advertisement_from_listener(&pair, &unsolicited).await;

    assert_eq!(
        await_receipt(&receipt).await,
        Err(RequestError::Receive),
        "the queued inbound failure fails the receipt, not the broadcasts alone"
    );
    assert_eq!(receipt.status(), RequestStatus::Failed);
    sizes_assert_link_still_answers(&pair).await;
}
