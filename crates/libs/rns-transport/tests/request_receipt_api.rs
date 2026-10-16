//! Black-box usage of the requester-side request types from outside the crate, in the shape
//! the README's "Sending requests" section shows: the public paths resolve, `RequestOptions`
//! builds with the documented builders and nothing else, the status and error enums carry
//! the documented variants (`RNS/Link.py:1318-1322`), `RequestError` is an error type a `?`
//! in the README snippet can convert, and the receipt is a `Clone + Send + Sync` handle a
//! consumer can move into another task.

use std::error::Error;
use std::fmt::Debug;
use std::time::Duration;

use rns_transport::destination::request::{
    RequestError, RequestOptions, RequestReceipt, RequestResponseData, RequestStatus,
};

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
