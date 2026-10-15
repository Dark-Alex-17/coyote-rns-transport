//! Black-box usage of the request handler registry from outside the crate, in the shape
//! the README's "Request handlers" section shows: the public paths resolve, a closure is a
//! handler through the blanket impl, a hand-written `RequestHandler` is accepted too, an
//! empty path is refused with `RnsError::InvalidArgument`, and `deregister_request_handler`
//! reports whether a handler was there (`RNS/Destination.py:381-412`).

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rand_core::OsRng;
use rns_transport::destination::request::{
    InboundRequest, RequestAllow, RequestHandler, RequestResponse,
};
use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::error::RnsError;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;

fn destination() -> SingleInputDestination {
    SingleInputDestination::new(
        PrivateIdentity::new_from_rand(OsRng),
        DestinationName::new("probe", "request-handler-api"),
    )
}

struct Silent;

impl RequestHandler for Silent {
    fn handle(
        &self,
        _request: InboundRequest,
    ) -> Pin<Box<dyn Future<Output = Option<RequestResponse>> + Send + '_>> {
        Box::pin(async { None })
    }
}

#[test]
fn readme_shaped_closure_registers_and_deregisters() {
    let mut destination = destination();

    // The README snippet, minus the transport mutex around the destination.
    destination
        .register_request_handler(
            "/echo",
            Arc::new(|request: InboundRequest| async move {
                Some(RequestResponse::Value(request.into_data()))
            }),
            RequestAllow::All,
            true,
        )
        .expect("the README registration compiles and succeeds");

    assert!(destination.deregister_request_handler("/echo"));
    assert!(!destination.deregister_request_handler("/echo"), "nothing left to remove");
    assert!(!destination.deregister_request_handler("/never"), "unknown paths report false");
}

#[test]
fn every_allow_mode_and_a_trait_impl_handler_are_accepted() {
    let mut destination = destination();
    let listed: HashSet<AddressHash> =
        HashSet::from([PrivateIdentity::new_from_rand(OsRng).as_identity().address_hash]);
    let handler: Arc<dyn RequestHandler> = Arc::new(Silent);

    destination
        .register_request_handler("/none", handler.clone(), RequestAllow::None, false)
        .expect("ALLOW_NONE registers");
    destination
        .register_request_handler("/all", handler.clone(), RequestAllow::All, true)
        .expect("ALLOW_ALL registers");
    destination
        .register_request_handler("/list", handler, RequestAllow::List(listed), true)
        .expect("ALLOW_LIST registers");

    assert!(destination.deregister_request_handler("/none"));
    assert!(destination.deregister_request_handler("/all"));
    assert!(destination.deregister_request_handler("/list"));
}

#[test]
fn empty_path_is_refused_and_registers_nothing() {
    let mut destination = destination();
    let outcome =
        destination.register_request_handler("", Arc::new(Silent), RequestAllow::All, true);
    assert!(matches!(outcome, Err(RnsError::InvalidArgument)), "got {outcome:?}");
    assert!(!destination.deregister_request_handler(""), "the refused path left no row");
}

#[test]
fn a_second_registration_replaces_the_first_without_error() {
    let mut destination = destination();
    destination
        .register_request_handler("/echo", Arc::new(Silent), RequestAllow::All, true)
        .expect("first registration");
    destination
        .register_request_handler("/echo", Arc::new(Silent), RequestAllow::None, false)
        .expect("second registration replaces the first");
    assert!(destination.deregister_request_handler("/echo"), "one row for the path");
    assert!(!destination.deregister_request_handler("/echo"), "and only one");
}
