use super::RequestId;
use crate::hash::AddressHash;
use crate::identity::Identity;

/// Slots in the queue between a link and the transport's request dispatch
/// worker. Python has no bound here (`RNS/Link.py:1004-1005` starts a
/// thread per request); this is a resource limit, and a full queue drops
/// the request with a debug log rather than blocking the link.
pub(crate) const REQUEST_DISPATCH_CAPACITY: usize = 1024;

/// How the request reached the link. Only the packet form is queued; its id
/// is the packet hash.
pub(crate) enum DispatchForm {
    Packet,
}

/// A decrypted request on its way from the link that received it to the
/// dispatch worker, which decodes it and finds the handler.
pub(crate) struct DispatchItem {
    pub(crate) link_id: AddressHash,
    pub(crate) destination_hash: AddressHash,
    pub(crate) request_id: RequestId,
    pub(crate) plain_text: Vec<u8>,
    pub(crate) remote_identity: Option<Identity>,
    pub(crate) form: DispatchForm,
}
