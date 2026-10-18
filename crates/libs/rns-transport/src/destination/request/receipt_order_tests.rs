// The exact status order of a receipt for each request form and response
// form, read off the `watch` after every transition so nothing coalesces:
// a packet request starts `Sent` (`RNS/Link.py:1348`), a request Resource
// goes `Delivered` when it completes (`:1372`), a response Resource goes
// `Receiving` at its advertisement (`:1420`) and either response form ends
// `Ready` (`:1442`). `Delivered` is a resource-form-only state: a packet
// request's response marks the underlying packet receipt (`:1445-1450`),
// never the request receipt.

use std::time::Duration;

use tokio::sync::watch;

use super::receipt_tests::{hash, id, response_segment};
use super::{PendingRequest, RequestStatus, ResponseArrival, ResponseFrame};

const TIMEOUT: Duration = Duration::from_secs(30);

/// Pushes the status on the `watch` if the subscriber has not seen it. The read goes
/// through the borrowed `Ref`, which reports a change after the entry concluded and
/// dropped the sender, where `Receiver::has_changed` would report the closed channel.
fn observed(status_rx: &mut watch::Receiver<RequestStatus>, seen: &mut Vec<RequestStatus>) {
    let latest = status_rx.borrow_and_update();
    if latest.has_changed() {
        seen.push(*latest);
    }
}

#[test]
fn a_packet_request_with_a_packet_response_goes_sent_then_ready() {
    let pending = PendingRequest::new(id(1), TIMEOUT, None);
    let receipt = pending.receipt();
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];

    let envelope = ResponseFrame::new(id(1), rmpv::Value::from("pong")).encode();
    assert!(pending
        .resolve(ResponseArrival::Packet { envelope: &envelope, response_size: 5 })
        .is_none());
    observed(&mut status_rx, &mut seen);

    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Ready]);
}

#[test]
fn a_packet_request_with_a_resource_response_goes_sent_receiving_ready() {
    let mut pending = PendingRequest::new(id(1), TIMEOUT, None);
    let receipt = pending.receipt();
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];

    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 1, 10, 5));
    observed(&mut status_rx, &mut seen);
    let envelope = ResponseFrame::new(id(1), rmpv::Value::from("pong")).encode();
    assert!(pending
        .resolve(ResponseArrival::Resource { data: &envelope, metadata: None })
        .is_none());
    observed(&mut status_rx, &mut seen);

    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Receiving, RequestStatus::Ready]);
}

#[test]
fn a_resource_request_with_a_packet_response_goes_sent_delivered_ready() {
    let pending = PendingRequest::new(id(2), TIMEOUT, Some(hash(0xAA)));
    let receipt = pending.receipt();
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];

    pending.mark_delivered();
    observed(&mut status_rx, &mut seen);
    let envelope = ResponseFrame::new(id(2), rmpv::Value::from("pong")).encode();
    assert!(pending
        .resolve(ResponseArrival::Packet { envelope: &envelope, response_size: 5 })
        .is_none());
    observed(&mut status_rx, &mut seen);

    assert_eq!(seen, vec![RequestStatus::Sent, RequestStatus::Delivered, RequestStatus::Ready]);
}

#[test]
fn a_resource_request_with_a_resource_response_goes_sent_delivered_receiving_ready() {
    let mut pending = PendingRequest::new(id(2), TIMEOUT, Some(hash(0xAA)));
    let receipt = pending.receipt();
    let mut status_rx = receipt.subscribe();
    let mut seen = vec![*status_rx.borrow_and_update()];

    pending.mark_delivered();
    observed(&mut status_rx, &mut seen);
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 1, 10, 5));
    observed(&mut status_rx, &mut seen);
    let envelope = ResponseFrame::new(id(2), rmpv::Value::from("pong")).encode();
    assert!(pending
        .resolve(ResponseArrival::Resource { data: &envelope, metadata: None })
        .is_none());
    observed(&mut status_rx, &mut seen);

    assert_eq!(
        seen,
        vec![
            RequestStatus::Sent,
            RequestStatus::Delivered,
            RequestStatus::Receiving,
            RequestStatus::Ready,
        ]
    );
}
