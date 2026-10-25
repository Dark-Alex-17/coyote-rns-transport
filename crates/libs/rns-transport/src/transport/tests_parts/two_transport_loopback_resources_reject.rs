// Loopback coverage for a destination-side advertisement reject. The reject is a Link
// packet the listener sends while it is still inside the inbound handler for the same
// link, so the test checks the transport is still alive afterwards: the same link carries
// a second Resource end to end and the listener's interface can still be stopped. Helpers
// come from the sibling `tests_parts` files included into this module.

/// Any request Resource larger than this is rejected by the listener's destination.
const REJECT_REQUEST_LIMIT: usize = 1024;
/// The reject fires on the advertised `d` (uncompressed size); random bytes keep `d` and
/// `t` close, so the advertisement reads the same whichever field is inspected.
const OVERSIZE_REQUEST_LEN: usize = 4 * 1024;
const ACCEPTED_REQUEST_LEN: usize = 256;
/// `stop_interface` locks the handler first; a wedged handler never returns from it.
const STOP_INTERFACE_TIMEOUT: Duration = Duration::from_secs(5);

/// The interface the listener read `link_id`'s traffic on. The tap fires before any handler
/// lock, so it is populated even when the handler itself is stuck.
fn tapped_link_iface(
    tap: &mut broadcast::Receiver<RxMessage>,
    link_id: AddressHash,
) -> AddressHash {
    loop {
        match tap.try_recv() {
            Ok(message) if message.packet.destination == link_id => return message.address,
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => continue,
            Err(_) => panic!("no link traffic reached the listener's interface tap"),
        }
    }
}

/// An oversize request Resource is rejected with a receiver cancel that the listener sends
/// from inside the link's inbound handler. The connector must learn of the rejection, the
/// same link must then carry an accepted request Resource, and the listener's interface
/// must still stop within a bounded time.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_transport_loopback_resources_reject_then_recover_on_same_link() {
    let pair = loopback_link_pair_with_request_limit("reject", Some(REJECT_REQUEST_LIMIT)).await;
    let mut sender_events = pair.connector.transport.resource_events();
    let mut receiver_events = pair.listener.transport.resource_events();
    let mut listener_tap = pair.listener.transport.iface_rx();

    let oversize = random_payload(OVERSIZE_REQUEST_LEN);
    let rejected_hash = pair
        .connector
        .transport
        .send_request_resource(&pair.link_id, b"loopback-oversize".to_vec(), oversize, None)
        .await
        .expect("oversize request resource dispatched");
    let rejected = await_outbound_terminal(&mut sender_events, rejected_hash).await;
    assert!(
        matches!(rejected, ResourceEventKind::OutboundRejected { .. }),
        "the oversize request is rejected, got {rejected:?}"
    );
    // The connector only hears the reject after the listener has sent it, so
    // anything the listener published for the rejected hash is already here.
    assert_no_event_for(&mut receiver_events, rejected_hash);

    let payload = random_payload(ACCEPTED_REQUEST_LEN);
    let request_id = b"loopback-accepted".to_vec();
    let resource_hash = pair
        .connector
        .transport
        .send_request_resource(&pair.link_id, request_id.clone(), payload.clone(), None)
        .await
        .expect("accepted request resource dispatched");
    let complete = await_resource_event(&mut receiver_events, |event| {
        event.hash == resource_hash && matches!(event.kind, ResourceEventKind::Complete(_))
    })
    .await;
    assert_eq!(complete.link_id, pair.link_id, "delivered on the link that saw the reject");
    let complete = complete_payload(complete);
    assert!(complete.is_request, "advertised as a request");
    assert_eq!(complete.request_id.as_deref(), Some(request_id.as_slice()));
    assert!(complete.data == payload, "received bytes match the sent payload");
    let outbound = await_outbound_terminal(&mut sender_events, resource_hash).await;
    assert!(
        matches!(outbound, ResourceEventKind::OutboundComplete),
        "the accepted request finishes cleanly, got {outbound:?}"
    );
    assert_no_event_for(&mut receiver_events, rejected_hash);

    let iface = tapped_link_iface(&mut listener_tap, pair.link_id);
    let stopped = timeout(STOP_INTERFACE_TIMEOUT, pair.listener.transport.stop_interface(iface))
        .await
        .expect("the listener's handler is not wedged after the reject");
    assert!(stopped, "the tapped interface was known to the listener");
}
