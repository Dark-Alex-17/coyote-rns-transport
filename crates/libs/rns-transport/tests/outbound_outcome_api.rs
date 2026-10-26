//! Black-box usage of the outbound outcome, link fast-fail and accept hook surface from
//! outside the crate, written from the README "Outbound resource outcomes" and "Opening
//! links and admitting resources" sections: the
//! new public paths resolve; `OutboundResult`, `OutboundFailure`, `RejectReason`,
//! `RnsError` and `ResourceEventKind` are `#[non_exhaustive]` (the wildcard arm a downstream
//! match needs is pinned by `#[deny(unreachable_patterns)]`); a snippet of the README's
//! `send_resource_with_outcome` shape compiles and, aimed at a link the transport does not
//! hold, returns `Err(InvalidArgument)` before any advertisement exists (the same answer the plain senders
//! gave before this release; `ConnectionError` is reserved for an advertisement that failed
//! to leave an interface the link does have); `Transport::link`
//! fails fast and repeatably with `NoPath` through the public API alone; the `send_resource` /
//! `send_request_resource` / `send_response_resource` argument lists from before this release
//! still compile; and `transport::ResourceAcceptHook` is the exported alias with the stated bounds.
//! The loopback behaviour behind each verdict lives in the crate's own `tests_parts` fixtures.

use std::sync::Arc;
use std::time::{Duration, Instant};

use rns_transport::destination::link::LinkStatus;
use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::error::RnsError;
use rns_transport::hash::{AddressHash, Hash};
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::resource::{OutboundFailure, OutboundResult, RejectReason, ResourceEventKind};
use rns_transport::transport::{ResourceAcceptHook, Transport, TransportConfig};

/// The README's `match` on an outcome, wildcard arm included.
#[deny(unreachable_patterns)]
fn outcome_name(result: &OutboundResult) -> &'static str {
    match result {
        OutboundResult::Complete => "complete",
        OutboundResult::Failed(_) => "failed",
        OutboundResult::Rejected(_) => "rejected",
        OutboundResult::Cancelled => "cancelled",
        _ => "unknown",
    }
}

#[deny(unreachable_patterns)]
fn failure_name(failure: &OutboundFailure) -> &'static str {
    match failure {
        OutboundFailure::NotSent => "not_sent",
        OutboundFailure::RetriesExhausted => "retries_exhausted",
        OutboundFailure::Segmenting => "segmenting",
        OutboundFailure::LinkClosed => "link_closed",
        _ => "unknown",
    }
}

#[deny(unreachable_patterns)]
fn reject_name(reason: &RejectReason) -> &'static str {
    match reason {
        RejectReason::PeerCancelled => "peer_cancelled",
        _ => "unknown",
    }
}

#[deny(unreachable_patterns)]
fn error_name(error: &RnsError) -> &'static str {
    match error {
        RnsError::OutOfMemory => "oom",
        RnsError::InvalidArgument => "invalid_argument",
        RnsError::IncorrectSignature => "incorrect_signature",
        RnsError::IncorrectHash => "incorrect_hash",
        RnsError::CryptoError => "crypto",
        RnsError::PacketError => "packet",
        RnsError::ConnectionError => "connection",
        RnsError::ResourceReader(_) => "resource_reader",
        RnsError::NoPath => "no_path",
        _ => "unknown",
    }
}

/// The shape every downstream `match` on `ResourceEventKind` has to take now: a wildcard arm.
#[deny(unreachable_patterns)]
fn resource_event_name(kind: &ResourceEventKind) -> &'static str {
    match kind {
        ResourceEventKind::Progress(_) => "progress",
        ResourceEventKind::SegmentComplete(_) => "segment_complete",
        ResourceEventKind::Complete(_) => "complete",
        ResourceEventKind::InboundFailed(_) => "inbound_failed",
        ResourceEventKind::OutboundComplete => "outbound_complete",
        ResourceEventKind::OutboundFailed => "outbound_failed",
        ResourceEventKind::OutboundRejected { .. } => "outbound_rejected",
        ResourceEventKind::OutboundCancelled => "outbound_cancelled",
        _ => "unknown",
    }
}

#[test]
fn outcome_enums_are_matchable_with_the_readme_wildcard_arm() {
    assert_eq!(outcome_name(&OutboundResult::Complete), "complete");
    assert_eq!(outcome_name(&OutboundResult::Cancelled), "cancelled");
    assert_eq!(outcome_name(&OutboundResult::Failed(OutboundFailure::LinkClosed)), "failed");
    assert_eq!(outcome_name(&OutboundResult::Rejected(RejectReason::PeerCancelled)), "rejected");
    assert_eq!(failure_name(&OutboundFailure::RetriesExhausted), "retries_exhausted");
    assert_eq!(reject_name(&RejectReason::PeerCancelled), "peer_cancelled");
    assert_eq!(error_name(&RnsError::NoPath), "no_path");
    assert_eq!(RnsError::NoPath.to_string(), "no path to destination");
    assert_eq!(RejectReason::PeerCancelled.to_string(), "the peer cancelled the resource");
}

#[test]
fn resource_event_kind_is_matchable_with_a_wildcard_arm() {
    assert_eq!(resource_event_name(&ResourceEventKind::OutboundComplete), "outbound_complete");
    assert_eq!(
        resource_event_name(&ResourceEventKind::OutboundRejected {
            reason: RejectReason::PeerCancelled
        }),
        "outbound_rejected"
    );
}

/// The README snippet's shape: the same call and the same five arms, each arm reduced to a
/// label where the README logs.
#[deny(clippy::await_holding_invalid_type)]
async fn readme_outcome_snippet(
    transport: &Transport,
    link_id: AddressHash,
    payload: Vec<u8>,
) -> Result<(Hash, &'static str), RnsError> {
    let (hash, outcome) =
        transport.send_resource_with_outcome(&link_id, payload, None, true).await?;
    let verdict = match outcome.await {
        OutboundResult::Complete => "proven",
        OutboundResult::Failed(_failure) => "failed",
        OutboundResult::Rejected(_reason) => "refused",
        OutboundResult::Cancelled => "cancelled here",
        _ => "a verdict this match predates",
    };
    Ok((hash, verdict))
}

fn random_link_id() -> AddressHash {
    PrivateIdentity::new_from_rand(OsRng).as_identity().address_hash
}

/// A link id the transport does not hold is refused as `InvalidArgument` before any
/// advertisement exists, on all three `_with_outcome` entry points alike, so there is no
/// outcome to await; `ConnectionError` is kept for an advertisement that failed to leave.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn readme_snippet_on_a_link_the_transport_does_not_hold_is_an_invalid_argument() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("outcome-api", &identity, false));

    let result = readme_outcome_snippet(&transport, random_link_id(), vec![0x5a; 512]).await;
    assert!(
        matches!(result, Err(RnsError::InvalidArgument)),
        "an unknown link is refused before any transfer exists, got {result:?}"
    );

    let request = transport
        .send_request_resource_with_outcome(
            &random_link_id(),
            b"req".to_vec(),
            vec![1; 64],
            None,
            None,
        )
        .await;
    assert!(matches!(request, Err(RnsError::InvalidArgument)), "got {request:?}");
    let response = transport
        .send_response_resource_with_outcome(
            &random_link_id(),
            b"req".to_vec(),
            vec![1; 64],
            None,
            true,
        )
        .await;
    assert!(matches!(response, Err(RnsError::InvalidArgument)), "got {response:?}");
    transport.stop().await;
}

/// The argument lists of the plain senders from before this release still compile and
/// still return `Result<Hash, RnsError>`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn plain_send_resource_signatures_are_unchanged() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("outcome-api-plain", &identity, false));
    let link_id = random_link_id();

    let plain: Result<Hash, RnsError> = transport.send_resource(&link_id, vec![1; 64], None).await;
    let request: Result<Hash, RnsError> =
        transport.send_request_resource(&link_id, b"req".to_vec(), vec![1; 64], None).await;
    let response: Result<Hash, RnsError> =
        transport.send_response_resource(&link_id, b"req".to_vec(), vec![1; 64], None).await;
    for result in [plain, request, response] {
        assert!(matches!(result, Err(RnsError::InvalidArgument)), "got {result:?}");
    }
    transport.stop().await;
}

fn unknown_destination() -> DestinationDesc {
    let remote = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    DestinationDesc {
        identity: remote,
        address_hash: remote.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    }
}

/// Through the public API alone, twice over: the fast fail is repeatable and leaves nothing
/// behind that would change the second answer.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn link_without_a_path_fails_fast_and_repeatably_when_broadcast_is_off() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("outcome-api-link", &identity, false));
    let destination = unknown_destination();

    for attempt in 0..2 {
        let started = Instant::now();
        let result = transport.link(destination).await;
        let elapsed = started.elapsed();
        assert!(
            matches!(result, Err(RnsError::NoPath)),
            "attempt {attempt}: expected Err(NoPath), got {:?}",
            result.as_ref().err()
        );
        assert!(elapsed < Duration::from_millis(500), "attempt {attempt} took {elapsed:?}");
    }
    transport.stop().await;
}

/// Broadcast on: the request goes out and the link is pending.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn link_without_a_path_is_pending_when_broadcast_is_on() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("outcome-api-bcast", &identity, true));

    let link = transport.link(unknown_destination()).await.expect("link request sent");
    assert_eq!(link.lock().await.status(), LinkStatus::Pending);
    transport.stop().await;
}

/// The alias is exported where the README says, with the bounds the README spells out, and
/// a closure of that shape installs without ceremony.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn resource_accept_hook_alias_is_exported_and_installable() {
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new("outcome-api-hook", &identity, false));

    let hook: ResourceAcceptHook =
        Arc::new(|_advertisement, _link_id, identity| identity.is_some());
    transport.set_resource_accept_hook(hook.clone()).await;
    transport.set_resource_accept_hook(hook).await;
    transport.stop().await;
}
