use core::fmt;
use std::sync::PoisonError;

use super::frame::RequestFrame;
use super::{OutcomeSlot, PathHash, RequestId, ResponseOutcomeReceiver, ResponseOutcomeTracker};
use crate::hash::AddressHash;
use crate::identity::Identity;

/// A request as a handler receives it: the six arguments Python passes to
/// `response_generator(path, data, request_id, link_id, remote_identity,
/// requested_at)` (`RNS/Link.py:831`), plus the path hash the request
/// carried.
pub struct InboundRequest {
    path: String,
    path_hash: PathHash,
    data: rmpv::Value,
    request_id: RequestId,
    link_id: AddressHash,
    remote_identity: Option<Identity>,
    requested_at: f64,
    outcome: OutcomeSlot,
}

impl InboundRequest {
    /// `frame` is the decoded `[requested_at, path_hash, data]` the requester
    /// sent (`RNS/Link.py:805-808`); `outcome` is the slot
    /// [`track_outcome`](Self::track_outcome) fills, of which the dispatcher
    /// keeps its own clone so it can resolve the verdict after `into_data`
    /// has consumed the request.
    pub(crate) fn new(
        path: String,
        frame: RequestFrame,
        request_id: RequestId,
        link_id: AddressHash,
        remote_identity: Option<Identity>,
        outcome: OutcomeSlot,
    ) -> Self {
        Self {
            path,
            path_hash: frame.path_hash(),
            requested_at: frame.requested_at(),
            data: frame.into_data(),
            request_id,
            link_id,
            remote_identity,
            outcome,
        }
    }

    /// The path the handler was registered under, `request_handler[0]`
    /// (`RNS/Link.py:812`).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The hash the request carried, which Python uses as the key into
    /// `destination.request_handlers` (`RNS/Link.py:807`, `:810`).
    pub fn path_hash(&self) -> PathHash {
        self.path_hash
    }

    /// The request body as the requester packed it, `unpacked_request[2]`
    /// (`RNS/Link.py:808`).
    pub fn data(&self) -> &rmpv::Value {
        &self.data
    }

    /// The `data` argument of `response_generator` (`RNS/Link.py:831`) by
    /// value; the owned twin of [`data`](Self::data).
    pub fn into_data(self) -> rmpv::Value {
        self.data
    }

    /// The `data` argument of `response_generator` (`RNS/Link.py:831`) by
    /// value, together with a [`ResponseOutcomeTracker`] over this request's
    /// outcome slot, for a handler that wants the data owned before it
    /// decides whether to watch the response. Calling
    /// [`ResponseOutcomeTracker::outcome`] has the same timing rule as
    /// [`track_outcome`](Self::track_outcome): arm it before the handler
    /// returns. A tracker dropped unarmed leaves the slot empty and the
    /// dispatcher resolves and logs nothing.
    ///
    /// Fork-only: Python's `handle_request` keeps nothing of the response it
    /// sends (`RNS/Link.py:846-852`).
    pub fn into_parts(self) -> (rmpv::Value, ResponseOutcomeTracker) {
        (self.data, ResponseOutcomeTracker::from_slot(self.outcome))
    }

    /// The `request_id` argument of `response_generator`
    /// (`RNS/Link.py:831`).
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// The `link_id` argument of `response_generator`, Python's
    /// `self.link_id` (`RNS/Link.py:831`).
    pub fn link_id(&self) -> AddressHash {
        self.link_id
    }

    /// The identity the peer delivered with `LINKIDENTIFY`, if it did:
    /// the `self.__remote_identity` argument of `response_generator`
    /// (`RNS/Link.py:831`).
    pub fn remote_identity(&self) -> Option<&Identity> {
        self.remote_identity.as_ref()
    }

    /// The requester's clock when it built the request, in seconds:
    /// `unpacked_request[0]` (`RNS/Link.py:806`).
    pub fn requested_at(&self) -> f64 {
        self.requested_at
    }

    /// A future that resolves with what became of this request's response.
    /// Call it inside the handler, before the handler returns and before
    /// [`into_data`](Self::into_data) consumes the request: the dispatcher
    /// takes the sender as the handler returns and resolves the future after
    /// that, so awaiting it inside the handler deadlocks that handler, and a
    /// sender installed once the handler has returned is never taken and
    /// resolves [`ResponseOutcome::NotSent`](super::ResponseOutcome::NotSent)
    /// only when the request is dropped. A second call replaces the first
    /// sender and the earlier receiver resolves `NotSent`.
    ///
    /// Fork-only: Python's `handle_request` keeps nothing of the response it
    /// sends (`RNS/Link.py:846-852`).
    pub fn track_outcome(&mut self) -> ResponseOutcomeReceiver {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *self.outcome.lock().unwrap_or_else(PoisonError::into_inner) = Some(tx);
        ResponseOutcomeReceiver::new(rx)
    }
}

impl fmt::Debug for InboundRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InboundRequest")
            .field("path", &self.path)
            .field("path_hash", &self.path_hash)
            .field("data", &self.data)
            .field("request_id", &self.request_id)
            .field("link_id", &self.link_id)
            .field("remote_identity", &self.remote_identity.map(|identity| identity.address_hash))
            .field("requested_at", &self.requested_at)
            .field(
                "outcome_tracked",
                &self.outcome.lock().unwrap_or_else(PoisonError::into_inner).is_some(),
            )
            .finish()
    }
}
