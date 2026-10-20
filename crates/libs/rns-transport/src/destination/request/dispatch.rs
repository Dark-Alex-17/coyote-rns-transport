use core::fmt;

use super::RequestId;
use crate::hash::AddressHash;
use crate::identity::Identity;

/// Slots in the queue between a link and the transport's request dispatch
/// worker. Python has no bound here (`RNS/Link.py:1004-1005` starts a
/// thread per request); this is a resource limit, and a full queue drops
/// the request with a debug log rather than blocking the link.
pub(crate) const REQUEST_DISPATCH_CAPACITY: usize = 1024;

/// How the request reached the link.
pub(crate) enum DispatchForm {
    /// One `REQUEST` packet; the id is the packet hash the link took
    /// (`RNS/Link.py:996`).
    Packet { request_id: RequestId },
    /// An assembled request resource (`RNS/Link.py:885-889`); the worker
    /// recomputes the id from the bytes, so the advertisement's `q` is
    /// never trusted.
    Resource,
}

impl fmt::Display for DispatchForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packet { request_id } => write!(f, "request {request_id}"),
            Self::Resource => f.write_str("resource-form request"),
        }
    }
}

/// A decrypted request on its way from the link that received it to the
/// dispatch worker, which decodes it and finds the handler.
pub(crate) struct DispatchItem {
    pub(crate) link_id: AddressHash,
    pub(crate) destination_hash: AddressHash,
    pub(crate) plain_text: Vec<u8>,
    pub(crate) remote_identity: Option<Identity>,
    pub(crate) form: DispatchForm,
}
