// The pending entry and the receipt on their own, without a link: the
// response clock (`RNS/Link.py:1331-1334`, `:1370-1373`, `:1389`), the size
// bookkeeping across a split response (`:1057-1064`, `:1016`), progress
// across segments (`RNS/Resource.py:1145-1200`), the metadata decode and
// what a receipt reads once its entry is gone.

use std::time::{Duration, Instant};

use super::receipt::ResponseDecodeFailure;
use super::{
    FrameDecodeError, PendingRequest, RequestError, RequestId, RequestOptions, RequestStatus,
    ResponseArrival, ResponseFrame, MAX_DEPTH, RESPONSE_GRACE,
};
use crate::hash::Hash;
use crate::resource::{ResourceAdvertisement, ResourceProgress, RANDOM_HASH_SIZE};

const TIMEOUT: Duration = Duration::from_secs(30);

fn id(byte: u8) -> RequestId {
    RequestId::new([byte; 16])
}

fn hash(byte: u8) -> Hash {
    Hash::new_from_slice(&[byte; 32])
}

fn packet_form() -> PendingRequest {
    PendingRequest::new(id(1), TIMEOUT, None)
}

fn resource_form() -> PendingRequest {
    PendingRequest::new(id(2), TIMEOUT, Some(hash(0xAA)))
}

/// Segment `segment_index` of `total_segments` of a response Resource whose
/// whole is `original`, advertising `data_size` and `transfer_size`.
fn response_segment(
    segment_hash: Hash,
    original: Hash,
    segment_index: u32,
    total_segments: u32,
    data_size: u64,
    transfer_size: u64,
) -> ResourceAdvertisement {
    ResourceAdvertisement {
        transfer_size,
        data_size,
        parts: 1,
        hash: segment_hash,
        random_hash: [0; RANDOM_HASH_SIZE],
        original_hash: original,
        segment_index,
        total_segments,
        request_id: Some(serde_bytes::ByteBuf::from(id(1).to_bytes().to_vec())),
        flags: 0,
        hashmap: Vec::new(),
    }
}

fn progress(received_bytes: u64, total_bytes: u64) -> ResourceProgress {
    ResourceProgress { received_bytes, total_bytes, received_parts: 0, total_parts: 0 }
}

#[test]
fn options_apply_what_the_builders_set() {
    let rtt = Duration::from_secs(1);
    let defaults = RequestOptions::default();
    assert_eq!(defaults.timeout_for_rtt(rtt), rtt * 6 + RESPONSE_GRACE, "rtt * 6 + 11.25 s");
    assert_eq!(defaults.max_response_size(), None, "no response size gate by default");

    let tuned =
        RequestOptions::default().with_timeout(Duration::from_secs(3)).with_max_response_size(4096);
    assert_eq!(tuned.timeout_for_rtt(rtt), Duration::from_secs(3), "the override ignores rtt");
    assert_eq!(tuned.max_response_size(), Some(4096));

    let last_wins = RequestOptions::default()
        .with_timeout(Duration::from_secs(1))
        .with_timeout(Duration::from_secs(2));
    assert_eq!(last_wins.timeout_for_rtt(rtt), Duration::from_secs(2));
}

#[test]
fn a_packet_form_request_starts_its_clock_when_sent() {
    let before = Instant::now();
    let pending = packet_form();
    let receipt = pending.receipt();
    let started = receipt.started_at().expect("the clock runs from the send");
    assert!(started >= before);
    assert_eq!(pending.deadline(), Some(started + TIMEOUT));
}

#[test]
fn a_resource_form_request_has_no_clock_until_delivered() {
    let pending = resource_form();
    let receipt = pending.receipt();
    assert_eq!(receipt.started_at(), None, "nothing proves the request yet");
    assert_eq!(pending.deadline(), None);

    let before = Instant::now();
    pending.mark_delivered();
    assert_eq!(receipt.status(), RequestStatus::Delivered);
    let started = receipt.started_at().expect("delivery starts the clock");
    assert!(started >= before);
    assert_eq!(pending.deadline(), Some(started + TIMEOUT));
}

#[test]
fn a_response_advertisement_starts_the_clock_if_delivery_has_not() {
    let mut pending = resource_form();
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 1, 10, 5));
    assert!(pending.receipt().started_at().is_some(), "`RNS/Link.py:1062-1063`");
}

#[test]
fn delivery_after_a_response_advertisement_changes_nothing() {
    let mut pending = resource_form();
    let receipt = pending.receipt();
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 1, 10, 5));
    let started = receipt.started_at().expect("the advertisement started the clock");
    pending.mark_delivered();
    assert_eq!(
        receipt.status(),
        RequestStatus::Receiving,
        "`RNS/Link.py:1372` would say Delivered"
    );
    assert_eq!(receipt.started_at(), Some(started));
}

#[test]
fn a_receiving_entry_has_no_deadline() {
    let mut pending = packet_form();
    assert!(pending.deadline().is_some());
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 1, 10, 5));
    assert_eq!(pending.status(), RequestStatus::Receiving);
    assert_eq!(pending.deadline(), None, "the response Resource's own failure ends it");
}

#[test]
fn a_timeout_that_does_not_fit_an_instant_never_expires() {
    let pending = PendingRequest::new(id(3), Duration::MAX, None);
    assert_eq!(pending.deadline(), None);
}

#[test]
fn response_sizes_accumulate_over_the_segments_of_a_split_response() {
    let mut pending = packet_form();
    let receipt = pending.receipt();
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 3, 3000, 1000));
    assert_eq!(receipt.response_size(), Some(3000));
    assert_eq!(receipt.response_transfer_size(), Some(1000));
    pending.mark_receiving(&response_segment(hash(2), hash(1), 2, 3, 3000, 1100));
    assert_eq!(receipt.response_size(), Some(3000), "`d` is read once");
    assert_eq!(receipt.response_transfer_size(), Some(2100), "`t` adds up");
    assert_eq!(pending.response_segment_hash(), Some(hash(2)));
    assert_eq!(pending.response_original_hash(), Some(hash(1)));
}

/// The sender re-advertises a segment it got no part request for
/// (`RNS/Resource.py:594-598`); Python counts the segment's `t` once
/// (`RNS/Resource.py:238-240`, `RNS/Link.py:1056`).
#[test]
fn a_repeated_advertisement_of_the_segment_in_flight_counts_once() {
    let mut pending = packet_form();
    let receipt = pending.receipt();
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 2, 3000, 1000));
    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 2, 3000, 1000));
    assert_eq!(receipt.response_transfer_size(), Some(1000), "`t` is counted once");
    assert_eq!(receipt.response_size(), Some(3000));
    pending.mark_receiving(&response_segment(hash(2), hash(1), 2, 2, 3000, 1100));
    assert_eq!(receipt.response_transfer_size(), Some(2100), "the next segment adds up");
    assert_eq!(pending.response_segment_hash(), Some(hash(2)));
}

#[test]
fn progress_counts_assembled_segments_as_whole_shares() {
    let mut pending = packet_form();
    let receipt = pending.receipt();
    pending.note_progress(&progress(5, 10));
    assert_eq!(receipt.progress(), 0.0, "no advertisement, no progress");

    pending.mark_receiving(&response_segment(hash(1), hash(1), 1, 4, 40, 40));
    pending.note_progress(&progress(5, 10));
    assert!((receipt.progress() - 0.125).abs() < f32::EPSILON, "{}", receipt.progress());
    pending.mark_receiving(&response_segment(hash(3), hash(1), 3, 4, 40, 40));
    pending.note_progress(&progress(10, 10));
    assert!((receipt.progress() - 0.75).abs() < f32::EPSILON, "{}", receipt.progress());
    pending.note_progress(&progress(0, 0));
    assert!((receipt.progress() - 0.75).abs() < f32::EPSILON, "an empty total changes nothing");
}

#[test]
fn a_packet_response_reports_its_packed_body_as_both_sizes() {
    let pending = packet_form();
    let receipt = pending.receipt();
    let envelope = ResponseFrame::new(id(1), rmpv::Value::from("pong")).encode();
    pending.resolve(ResponseArrival::Packet { envelope: &envelope, response_size: 5 });
    assert_eq!(receipt.status(), RequestStatus::Ready);
    assert_eq!(receipt.response_size(), Some(5));
    assert_eq!(receipt.response_transfer_size(), Some(5));
    assert_eq!(receipt.progress(), 1.0);
}

#[test]
fn failing_a_receiving_entry_hands_back_the_segment_to_cancel() {
    let mut pending = packet_form();
    let receipt = pending.receipt();
    assert_eq!(packet_form().fail(RequestError::Timeout), None, "nothing inbound to cancel");
    pending.mark_receiving(&response_segment(hash(7), hash(6), 2, 2, 10, 5));
    assert_eq!(pending.fail(RequestError::LinkClosed), Some(hash(7)));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert!(receipt.concluded_at().is_some());
}

#[test]
fn metadata_nested_deeper_than_the_frame_limit_is_refused() {
    let shallow = vec![0x91; MAX_DEPTH - 1];
    let mut shallow = shallow;
    shallow.push(0xc0);
    let decoded = ResponseArrival::Resource { data: b"file", metadata: Some(&shallow) }
        .decode()
        .expect("depth within the limit decodes");
    assert_eq!(decoded.data(), Some(b"file".as_slice()));

    let mut deep = vec![0x91; MAX_DEPTH + 1];
    deep.push(0xc0);
    assert!(
        matches!(
            ResponseArrival::Resource { data: b"file", metadata: Some(&deep) }.decode(),
            Err(ResponseDecodeFailure::Metadata(FrameDecodeError::DepthExceeded))
        ),
        "depth over the limit is refused"
    );
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_response_that_does_not_decode_fails_decode_and_hands_back_the_cause() {
    let pending = packet_form();
    let receipt = pending.receipt();
    let cause = pending.resolve(ResponseArrival::Packet { envelope: &[0xc0], response_size: 1 });
    assert!(matches!(cause, Some(ResponseDecodeFailure::Frame(_))), "nil is not an envelope");
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(receipt.response().await, Err(RequestError::Decode));
}

/// The entry is dropped without concluding, as a link that vanished would
/// drop it: `status()` and `concluded_at()` conclude it `LinkClosed` on
/// their own, and `response()` returns the same.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_receipt_whose_entry_vanished_concludes_link_closed() {
    let pending = packet_form();
    let receipt = pending.receipt();
    drop(pending);
    assert_eq!(receipt.status(), RequestStatus::Failed);
    let concluded = receipt.concluded_at().expect("a gone entry concludes the receipt");
    assert_eq!(receipt.response().await, Err(RequestError::LinkClosed));
    assert_eq!(receipt.status(), RequestStatus::Failed);
    assert_eq!(receipt.concluded_at(), Some(concluded), "concluded once");
    assert_eq!(receipt.clone().response().await, Err(RequestError::LinkClosed));
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn subscribe_reports_changes_after_the_call_only() {
    let pending = resource_form();
    let receipt = pending.receipt();
    pending.mark_delivered();
    let mut status_rx = receipt.subscribe();
    assert!(!status_rx.has_changed().expect("the entry is alive"), "`Delivered` predates it");
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Delivered);
    pending.fail(RequestError::Timeout);
    assert_eq!(*status_rx.borrow_and_update(), RequestStatus::Failed);
}
