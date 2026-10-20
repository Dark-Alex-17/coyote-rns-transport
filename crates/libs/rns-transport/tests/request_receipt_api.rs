//! Black-box usage of the requester-side request types from outside the crate, in the shape
//! the README's "Sending requests" section shows: the public paths resolve, `RequestOptions`
//! builds with the documented builders, the status and error enums carry
//! the documented variants (`RNS/Link.py:1318-1322`), `RequestError` is an error type a `?`
//! in the README snippet can convert, and the receipt is a `Clone + Send + Sync` handle a
//! consumer can move into another task.

use std::error::Error;
use std::fmt::Debug;
use std::time::Duration;

use rns_transport::destination::request::{
    RequestAllow, RequestError, RequestOptions, RequestReceipt, RequestResponse,
    RequestResponseData, RequestStatus,
};
use rns_transport::transport::AnnounceEvent;

fn assert_clone_send_sync<T: Clone + Send + Sync + 'static>() {}
fn assert_send_sync<T: Send + Sync + 'static>() {}
fn assert_clone_default_debug<T: Clone + Default + Debug>() {}

#[test]
fn readme_shaped_options_build_from_the_default_and_the_builders() {
    assert_clone_default_debug::<RequestOptions>();
    let tuned: RequestOptions =
        RequestOptions::default().with_timeout(Duration::from_secs(3)).with_max_response_size(4096);
    let copy = tuned.clone();
    assert!(!format!("{copy:?}").is_empty(), "the options render for a consumer's log line");
}

#[test]
fn status_and_error_variants_match_the_documented_set() {
    // Python `RequestReceipt`: FAILED=0 SENT=1 DELIVERED=2 RECEIVING=3 READY=4.
    let statuses = [
        RequestStatus::Failed,
        RequestStatus::Sent,
        RequestStatus::Delivered,
        RequestStatus::Receiving,
        RequestStatus::Ready,
    ];
    for (index, status) in statuses.iter().enumerate() {
        for (other_index, other) in statuses.iter().enumerate() {
            assert_eq!(status == other, index == other_index, "{status:?} vs {other:?}");
        }
    }

    let errors = [
        RequestError::Timeout,
        RequestError::LinkClosed,
        RequestError::Rejected,
        RequestError::Oversize,
        RequestError::Decode,
        RequestError::Send,
        RequestError::Receive,
    ];
    for error in errors {
        let boxed: Box<dyn Error + Send + Sync> = Box::new(error);
        assert!(!boxed.to_string().is_empty(), "{error:?} renders a message");
        let copied = error;
        assert_eq!(copied, error, "errors compare by variant");
    }
}

#[test]
fn receipt_and_response_are_handles_a_consumer_can_share() {
    assert_clone_send_sync::<RequestReceipt>();
    assert_clone_send_sync::<RequestResponseData>();
    assert_send_sync::<RequestOptions>();
    assert_send_sync::<RequestError>();
    assert_send_sync::<RequestStatus>();
}

/// The README says "`RequestError` is `#[non_exhaustive]`" and the CHANGELOG lists the other
/// request-layer enums the same way: outside the crate, a match naming every documented
/// variant still needs a wildcard arm, and `#[deny(unreachable_patterns)]` makes that arm a
/// compile error the moment an enum stops being non-exhaustive.
#[deny(unreachable_patterns)]
fn request_error_name(error: &RequestError) -> &'static str {
    match error {
        RequestError::Timeout => "timeout",
        RequestError::LinkClosed => "link closed",
        RequestError::Rejected => "rejected",
        RequestError::Oversize => "oversize",
        RequestError::Decode => "decode",
        RequestError::Send => "send",
        RequestError::Receive => "receive",
        _ => "an error this match predates",
    }
}

#[deny(unreachable_patterns)]
fn request_status_name(status: &RequestStatus) -> &'static str {
    match status {
        RequestStatus::Sent => "sent",
        RequestStatus::Delivered => "delivered",
        RequestStatus::Receiving => "receiving",
        RequestStatus::Ready => "ready",
        RequestStatus::Failed => "failed",
        _ => "a status this match predates",
    }
}

#[deny(unreachable_patterns)]
fn request_allow_name(allow: &RequestAllow) -> &'static str {
    match allow {
        RequestAllow::None => "none",
        RequestAllow::All => "all",
        RequestAllow::List(_) => "list",
        _ => "a policy this match predates",
    }
}

#[deny(unreachable_patterns)]
fn request_response_name(response: &RequestResponse) -> &'static str {
    match response {
        RequestResponse::Value(_) => "value",
        RequestResponse::Bytes(_) => "bytes",
        RequestResponse::Resource { data: _, metadata: _ } => "resource",
        _ => "a response form this match predates",
    }
}

/// `AnnounceEvent` is `#[non_exhaustive]`: a consumer destructures it with `..` and reads the
/// documented `is_path_response` flag; a struct literal is no longer written outside the crate.
fn announce_is_path_response(event: &AnnounceEvent) -> bool {
    let AnnounceEvent { is_path_response, hops: _, .. } = event;
    *is_path_response
}

#[test]
fn request_layer_enums_are_non_exhaustive_outside_the_crate() {
    assert_eq!(request_error_name(&RequestError::Timeout), "timeout");
    assert_eq!(request_status_name(&RequestStatus::Sent), "sent");
    assert_eq!(request_allow_name(&RequestAllow::All), "all");
    assert_eq!(request_response_name(&RequestResponse::Bytes(Vec::new())), "bytes");
    let _reads_the_flag: fn(&AnnounceEvent) -> bool = announce_is_path_response;
}
