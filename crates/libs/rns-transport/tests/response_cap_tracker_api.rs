//! Consumer-side pins over the responder's response cap and the `into_parts` tracker, read
//! from outside the crate the way a handler author sees them: the three cap accessors on a
//! fresh `SingleInputDestination` start unbounded and round-trip a value, setting zero is
//! accepted (every response then measures over it), `ResponseOutcome::TooLarge { len, max }`
//! destructures by field name and compares with `PartialEq`, and `ResponseOutcomeTracker` is
//! `Send + Sync`, implements `Debug` and is deliberately not `Clone`, so a handler
//! cannot arm the same slot twice through two handles.

use std::marker::PhantomData;

use rand_core::OsRng;
use rns_transport::destination::request::{ResponseOutcome, ResponseOutcomeTracker};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::identity::PrivateIdentity;
use rns_transport::resource::OutboundResult;

fn destination() -> SingleInputDestination {
    SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("probe", "response-cap"),
    )
}

#[test]
fn a_fresh_destination_has_no_response_cap_and_round_trips_one() {
    let mut destination = destination();
    assert_eq!(destination.max_response_size(), None, "unbounded until a cap is set");
    assert_eq!(destination.max_request_size(), None, "the request cap is a separate knob");

    destination.set_max_response_size(1024).expect("a positive cap is accepted");
    assert_eq!(destination.max_response_size(), Some(1024));
    assert_eq!(destination.max_request_size(), None, "setting one cap leaves the other alone");

    destination.set_max_response_size(2048).expect("a second set replaces the first");
    assert_eq!(destination.max_response_size(), Some(2048));

    destination.clear_max_response_size();
    assert_eq!(destination.max_response_size(), None);
    destination.clear_max_response_size();
    assert_eq!(destination.max_response_size(), None, "clearing twice is harmless");
}

/// The request-cap twin accepts zero, so the response cap does too: a frame is never
/// empty, so zero means every response is refused, which is a legitimate way to mute a
/// destination's answers without deregistering its handlers.
#[test]
fn a_zero_response_cap_is_accepted_like_the_request_cap() {
    let mut destination = destination();
    destination.set_max_request_size(0).expect("the request cap accepts zero");
    destination.set_max_response_size(0).expect("the response cap accepts zero");
    assert_eq!(destination.max_response_size(), Some(0));
}

#[test]
fn too_large_destructures_by_field_and_compares() {
    let outcome = ResponseOutcome::TooLarge { len: 513, max: 512 };
    let ResponseOutcome::TooLarge { len, max } = outcome else {
        panic!("the variant exposes len and max");
    };
    assert_eq!((len, max), (513, 512));
    assert_eq!(outcome, ResponseOutcome::TooLarge { len: 513, max: 512 });
    assert_ne!(outcome, ResponseOutcome::TooLarge { len: 512, max: 512 });
    assert_ne!(outcome, ResponseOutcome::NotSent);
    assert_ne!(outcome, ResponseOutcome::Resource(OutboundResult::Complete));

    // The enum is `#[non_exhaustive]`, so a consumer match needs a wildcard arm; the
    // four known variants are each reachable by name.
    let describe = |outcome: &ResponseOutcome| match outcome {
        ResponseOutcome::Packet => "packet",
        ResponseOutcome::Resource(_) => "resource",
        ResponseOutcome::NotSent => "not sent",
        ResponseOutcome::TooLarge { .. } => "too large",
        _ => "unknown",
    };
    assert_eq!(describe(&outcome), "too large");
}

/// Picks the inherent `IS_CLONE` when `T: Clone` and the trait's `false` otherwise, so the
/// absence of a `Clone` impl is observable from outside the crate without a compile-fail
/// harness.
trait NotClone {
    const IS_CLONE: bool = false;
}
impl<T> NotClone for Probe<T> {}
struct Probe<T>(PhantomData<T>);
impl<T: Clone> Probe<T> {
    const IS_CLONE: bool = true;
}

#[test]
fn the_tracker_is_send_sync_debug_and_not_clone() {
    fn assert_send_sync<T: Send + Sync + std::fmt::Debug>() {}
    assert_send_sync::<ResponseOutcomeTracker>();
    // Compile-time: a tracker is a single handle on the slot and must not be `Clone`; the
    // positive control shows the probe does see a `Clone` impl when there is one.
    const {
        assert!(!<Probe<ResponseOutcomeTracker>>::IS_CLONE);
        assert!(<Probe<ResponseOutcome>>::IS_CLONE);
    }
}
