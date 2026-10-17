// Usage-pattern coverage for the end of a request record's life and for a response Resource
// whose advertised data size was a lie. A record a `RequestReceipt` owns and a receipt-less
// one alike stop soliciting once the response Resource they admitted has concluded, whether
// it completed or the responder cancelled it (`RNS/Link.py:1389` suspends the clock while a
// response is inbound, `:1437-1443` resolve the receipt on completion and `:1046-1047` then
// drop a second response as one nothing on the link sent). Python measures
// `max_response_size` against the advertised `d` (`RNS/Link.py:1046-1049`) and trusts it
// (`:1049`); this crate measures the assembled bytes once more, because `d` is the peer's
// claim, and an assembled response over the limit fails the receipt `Oversize`. The
// boundary of the legacy-id arm is inclusive: a response of exactly the limit is delivered.
// Helpers come from the sibling `tests_parts` files.

/// Bytes the legacy-id boundary tests measure against.
const SIZES_PROBE5_LIMIT: usize = 2048;

/// A one-part response Resource under the loopback MTU: the tests that complete or drop a
/// response need it advertised and assembled, not large.
const SIZES_PROBE5_SMALL_RESPONSE_LEN: usize = 256;

/// The response the responder cancels; still one part under the loopback MTU, so it is the
/// hold below, not this length, that puts the cancel behind the advertisement before any
/// part is pulled.
const SIZES_PROBE5_CANCELLED_RESPONSE_LEN: usize = 16 * 1024;

/// How long the connector is held so the advertisement and the cancel behind it are read
/// back to back once it resumes.
const SIZES_PROBE5_HOLD: Duration = Duration::from_secs(2);

/// Waits for the first response advertisement for `request_id` the connector's tap sees and
/// returns it decrypted, so a test can re-advertise the same Resource under other terms.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe5_capture_response_advertisement(
    pair: &mut LoopbackRequestPair,
    request_id: &[u8],
) -> ResourceAdvertisement {
    timeout(RESPONSE_TIMEOUT, async {
        loop {
            match pair.connector_tap.recv().await {
                Ok(message)
                    if message.packet.destination == pair.handles.link_id
                        && message.packet.context == PacketContext::ResourceAdvrtisement =>
                {
                    let advertisement = {
                        let link = pair.handles.link.lock().await;
                        decrypt_resource_advertisement(&link, &message.packet)
                    };
                    if advertisement.request_id.as_deref().map(Vec::as_slice) == Some(request_id) {
                        return advertisement;
                    }
                }
                Ok(_) => {}
                Err(RecvError::Lagged(skipped)) => {
                    panic!(
                        "interface tap lagged by {skipped} packets; the advertisement may be lost"
                    )
                }
                Err(RecvError::Closed) => panic!("interface tap closed"),
            }
        }
    })
    .await
    .expect("the response advertisement reaches the connector")
}

/// No response Resource under the raw id `request_id` is assembled on `link_id` within
/// `SIZES_SILENCE`, and no inbound Resource fails there.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe5_assert_no_response_under(
    events: &mut broadcast::Receiver<ResourceEvent>,
    link_id: AddressHash,
    request_id: &[u8],
) {
    let deadline = Instant::now() + SIZES_SILENCE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        match timeout(remaining, events.recv()).await {
            Ok(Ok(event)) if event.link_id == link_id => match &event.kind {
                ResourceEventKind::Complete(complete) => assert!(
                    complete.request_id.as_deref() != Some(request_id),
                    "a response the record no longer solicits was assembled"
                ),
                ResourceEventKind::InboundFailed(failure) => {
                    panic!("an inbound Resource failed on the link: {failure:?}")
                }
                _ => {}
            },
            Ok(Ok(_)) => {}
            Ok(Err(RecvError::Lagged(_))) => continue,
            Ok(Err(RecvError::Closed)) | Err(_) => return,
        }
    }
}

/// Solicits a response under `request_id` from the connector and waits for the request
/// Resource to be delivered, from when the record's clock runs.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe5_solicit_and_deliver(
    handles: &LoopbackLinkHandles,
    events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
) {
    let request_hash =
        solicit_response(&handles.connector.transport, &handles.link_id, request_id).await;
    await_resource_event(events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
}

/// Sends `len` random bytes from the listener as a response under `request_id` and waits
/// for the connector to assemble them, returning the assembled response.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe5_respond_and_complete(
    handles: &LoopbackLinkHandles,
    events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
    len: usize,
) -> ResourceComplete {
    let payload = random_payload(len);
    let resource_hash = handles
        .listener
        .transport
        .send_response_resource(&handles.link_id, request_id.to_vec(), payload.clone(), None)
        .await
        .expect("response resource dispatched");
    let complete = await_resource_event(events, |event| {
        event.hash == resource_hash && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    let complete = complete_payload(complete);
    assert!(complete.is_response, "advertised as a response");
    assert_eq!(complete.request_id.as_deref(), Some(request_id));
    assert!(complete.data == payload, "received bytes match the sent payload");
    complete
}

/// A second response under `request_id` after the record is gone: sent from the listener,
/// never assembled by the connector and never rejected either.
#[deny(clippy::await_holding_invalid_type)]
async fn sizes_probe5_assert_second_response_is_dropped(
    handles: &LoopbackLinkHandles,
    events: &mut broadcast::Receiver<ResourceEvent>,
    request_id: &[u8],
) {
    let mut listener_tap = handles.listener.transport.iface_rx();
    handles
        .listener
        .transport
        .send_response_resource(
            &handles.link_id,
            request_id.to_vec(),
            random_payload(SIZES_PROBE5_SMALL_RESPONSE_LEN),
            None,
        )
        .await
        .expect("second response resource dispatched");
    sizes_probe5_assert_no_response_under(events, handles.link_id, request_id).await;
    assert_eq!(
        count_link_packets(
            &mut listener_tap,
            handles.link_id,
            PacketContext::ResourceReceiverCancel
        ),
        0,
        "a response nothing solicits any more is dropped without a reject"
    );
}

/// A receipt bounded its response; the responder's Resource advertises a data size under
/// the bound but assembles over it. The advertisement is admitted on its claim, the
/// assembled bytes are measured again, the receipt fails `Oversize`, and the assembled
/// `Complete` is withheld from `resource_events()` subscribers; the link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe5_response_assembling_over_its_advertised_size_fails_oversize(
) {
    let mut pair = loopback_request_pair("sizes-probe5-lying-advertisement").await;
    let link_id = pair.handles.link_id;
    register_handler(&pair, "/quiet", RequestAllow::All, silent_handler()).await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut connector_events = pair.handles.connector.transport.resource_events();

    let receipt = request_with(
        &pair,
        "/quiet",
        rmpv::Value::Nil,
        RequestOptions::default().with_max_response_size(SIZES_TIGHT_LIMIT),
    )
    .await;
    let mut status_rx = receipt.subscribe();

    // The listener advertises a real file response, over the limit, under an id nothing
    // solicited; the connector drops that advertisement and the listener keeps the Resource
    // ready to serve. The same Resource is then re-advertised under the receipt's id with a
    // data size the limit allows. The payload is random, so the Resource goes uncompressed:
    // the receiver caps decompression by the advertised `d` (`resource/receiver.rs`,
    // `max_decompressed_resource_size`), and a compressed lie would fail there instead of
    // reaching the assembled-bytes measurement this test is about.
    let decoy_id = b"sizes-probe5-decoy".to_vec();
    let metadata =
        rmpv::Value::Map(vec![(rmpv::Value::from("name"), rmpv::Value::from("big.bin"))]);
    let mut packed_metadata = Vec::new();
    rmpv::encode::write_value(&mut packed_metadata, &metadata).expect("metadata packs");
    pair.handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            decoy_id.clone(),
            random_payload(SIZES_TIGHT_LIMIT + 64),
            Some(packed_metadata),
        )
        .await
        .expect("decoy response resource dispatched");
    let mut advertisement = sizes_probe5_capture_response_advertisement(&mut pair, &decoy_id).await;
    assert!(
        advertisement.data_size as usize > SIZES_TIGHT_LIMIT,
        "the real advertisement is over the limit"
    );
    advertisement.request_id =
        Some(serde_bytes::ByteBuf::from(receipt.request_id().to_bytes().to_vec()));
    advertisement.data_size = (SIZES_TIGHT_LIMIT / 2) as u64;
    inject_advertisement_from_listener(&pair, &advertisement).await;

    await_status(&mut status_rx, RequestStatus::Receiving, RECEIPT_TIMEOUT).await;
    assert_eq!(
        await_receipt(&receipt).await,
        Err(RequestError::Oversize),
        "the assembled bytes are over the limit the advertisement claimed to respect"
    );
    assert_eq!(receipt.status(), RequestStatus::Failed);
    sizes_assert_no_response_activity(&mut connector_events, link_id, receipt.request_id()).await;
    sizes_assert_link_still_answers(&pair).await;
}

/// A receipt-less `send_request_resource` record: once the response Resource it admitted
/// has completed, a second response under the same id is dropped without a reject, and a
/// fresh request on the link is still answered.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe5_completed_response_ends_a_receipt_less_record(
) {
    let handles = loopback_link_handles("sizes-probe5-completed-record").await;
    let mut events = handles.connector.transport.resource_events();
    let request_id = b"sizes-probe5-done".to_vec();

    sizes_probe5_solicit_and_deliver(&handles, &mut events, &request_id).await;
    sizes_probe5_respond_and_complete(
        &handles,
        &mut events,
        &request_id,
        SIZES_PROBE5_SMALL_RESPONSE_LEN,
    )
    .await;
    sizes_probe5_assert_second_response_is_dropped(&handles, &mut events, &request_id).await;

    let again = b"sizes-probe5-again".to_vec();
    sizes_probe5_solicit_and_deliver(&handles, &mut events, &again).await;
    sizes_probe5_respond_and_complete(
        &handles,
        &mut events,
        &again,
        SIZES_PROBE5_SMALL_RESPONSE_LEN,
    )
    .await;
}

/// A receipt-less record whose admitted response the responder cancels: the connector reads
/// the advertisement and the cancel back to back, reports the transfer `InboundFailed`, and
/// that outcome ends the record, so a second response under the id is dropped; a fresh id
/// on the link is still served.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe5_cancelled_response_ends_a_receipt_less_record(
) {
    let handles = loopback_link_handles("sizes-probe5-cancelled-record").await;
    let link_id = handles.link_id;
    let mut events = handles.connector.transport.resource_events();
    let request_id = b"sizes-probe5-cancel".to_vec();
    sizes_probe5_solicit_and_deliver(&handles, &mut events, &request_id).await;

    // Held before the response goes out, the connector reads the advertisement, admits it,
    // asks for parts, and then reads the cancel the responder sent right behind it.
    let hold = stall_transport_for(&handles.connector.transport, SIZES_PROBE5_HOLD).await;
    let resource_hash = handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            request_id.clone(),
            random_payload(SIZES_PROBE5_CANCELLED_RESPONSE_LEN),
            None,
        )
        .await
        .expect("response resource dispatched");
    let cancelled = handles
        .listener
        .transport
        .cancel_resource(&link_id, resource_hash)
        .await
        .expect("the responder can cancel its outbound response");
    assert!(cancelled, "the cancel reaches the wire");
    hold.await.expect("hold task finished");

    let failed = await_resource_event(&mut events, |event| {
        event.link_id == link_id
            && match &event.kind {
                ResourceEventKind::InboundFailed(_) => true,
                ResourceEventKind::Complete(complete) => {
                    panic!("the cancelled response was assembled: {:?}", complete.request_id)
                }
                _ => false,
            }
    })
    .await;
    assert_eq!(failed.hash, resource_hash, "the cancelled transfer is reported under its hash");

    sizes_probe5_assert_second_response_is_dropped(&handles, &mut events, &request_id).await;

    let again = b"sizes-probe5-after-cancel".to_vec();
    sizes_probe5_solicit_and_deliver(&handles, &mut events, &again).await;
    sizes_probe5_respond_and_complete(
        &handles,
        &mut events,
        &again,
        SIZES_PROBE5_SMALL_RESPONSE_LEN,
    )
    .await;
}

/// A bare `Link::request_packet` record: once the response Resource its handler sent has
/// completed, a second response under the same id is dropped without a reject, and the
/// link still answers.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe5_completed_response_ends_a_bare_request_packet_record(
) {
    let pair = loopback_request_pair("sizes-probe5-bare-packet-record").await;
    let link_id = pair.handles.link_id;
    let body = over_mdu_body(&pair).await;
    let served = body.clone();
    register_handler(&pair, "/big", RequestAllow::All, move |_: InboundRequest| {
        std::future::ready(Some(RequestResponse::Bytes(served.clone())))
    })
    .await;
    register_handler(&pair, "/echo", RequestAllow::All, echo_handler()).await;
    let mut events = pair.handles.connector.transport.resource_events();

    let id = send_request(&pair, "/big", rmpv::Value::Nil).await;
    let complete = await_response_resource(&mut events, link_id, id).await;
    let (answered, value) = unpack_response_envelope(&complete.data).expect("response envelope");
    assert_eq!(answered, id.to_bytes(), "the response names the request");
    assert_eq!(value, rmpv::Value::Binary(body));

    let mut listener_tap = pair.handles.listener.transport.iface_rx();
    pair.handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            id.to_bytes().to_vec(),
            random_payload(SIZES_PROBE5_SMALL_RESPONSE_LEN),
            None,
        )
        .await
        .expect("second response resource dispatched");
    sizes_assert_no_response_activity(&mut events, link_id, id).await;
    assert_eq!(
        count_link_packets(&mut listener_tap, link_id, PacketContext::ResourceReceiverCancel),
        0,
        "a response to a request already answered is dropped without a reject"
    );
    sizes_assert_link_still_answers(&pair).await;
}

/// The legacy-id limit is inclusive on both measurements: a response Resource of exactly
/// `max_response_size` bytes is admitted at its advertisement and delivered after assembly,
/// while one byte more is refused at the advertisement and the responder sees
/// `OutboundRejected`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_request_resource_form_sizes_probe5_legacy_id_limit_is_inclusive() {
    let handles = loopback_link_handles("sizes-probe5-legacy-boundary").await;
    let link_id = handles.link_id;
    let mut events = handles.connector.transport.resource_events();
    let mut responder_events = handles.listener.transport.resource_events();

    let at_limit_id = b"sizes-probe5-at-limit".to_vec();
    let request_hash = handles
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &link_id,
            at_limit_id.clone(),
            random_payload(64),
            None,
            Some(SIZES_PROBE5_LIMIT),
        )
        .await
        .expect("bounded request resource dispatched");
    await_resource_event(&mut events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
    let complete =
        sizes_probe5_respond_and_complete(&handles, &mut events, &at_limit_id, SIZES_PROBE5_LIMIT)
            .await;
    assert_eq!(complete.data.len(), SIZES_PROBE5_LIMIT, "exactly the limit is delivered");

    let over_limit_id = b"sizes-probe5-over-limit".to_vec();
    let request_hash = handles
        .connector
        .transport
        .send_request_resource_with_max_response_size(
            &link_id,
            over_limit_id.clone(),
            random_payload(64),
            None,
            Some(SIZES_PROBE5_LIMIT),
        )
        .await
        .expect("bounded request resource dispatched");
    await_resource_event(&mut events, |event| {
        event.hash == request_hash && matches!(event.kind, ResourceEventKind::OutboundComplete)
    })
    .await;
    let rejected_hash = handles
        .listener
        .transport
        .send_response_resource(
            &link_id,
            over_limit_id.clone(),
            random_payload(SIZES_PROBE5_LIMIT + 1),
            None,
        )
        .await
        .expect("over-limit response resource dispatched");
    let outcome = await_resource_event(&mut responder_events, |event| {
        event.hash == rejected_hash
            && matches!(
                event.kind,
                ResourceEventKind::OutboundRejected
                    | ResourceEventKind::OutboundComplete
                    | ResourceEventKind::OutboundFailed
            )
    })
    .await;
    assert!(
        matches!(outcome.kind, ResourceEventKind::OutboundRejected),
        "one byte over the limit is refused at the advertisement, got {:?}",
        outcome.kind
    );
    sizes_probe5_assert_no_response_under(&mut events, link_id, &over_limit_id).await;
}
