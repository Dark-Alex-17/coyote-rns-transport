use core::fmt;

use super::{PathHash, RequestId};
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
}

impl InboundRequest {
    pub(crate) fn new(
        path: String,
        path_hash: PathHash,
        data: rmpv::Value,
        request_id: RequestId,
        link_id: AddressHash,
        remote_identity: Option<Identity>,
        requested_at: f64,
    ) -> Self {
        Self { path, path_hash, data, request_id, link_id, remote_identity, requested_at }
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
            .finish()
    }
}
