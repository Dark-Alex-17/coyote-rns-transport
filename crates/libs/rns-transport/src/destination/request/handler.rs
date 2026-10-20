use core::future::Future;
use core::pin::Pin;
use std::sync::Arc;

use super::{InboundRequest, RequestAllow};

/// What a handler answers with; `None` sends nothing (`RNS/Link.py:844`).
///
/// `Value` goes on the wire as `packb([request_id, response])`
/// (`RNS/Link.py:848`). `Bytes` is the same envelope with the body packed as
/// msgpack `bin`, which is how Python packs a `bytes` response. Either one
/// leaves as a single `RESPONSE` packet when the envelope fits the link MDU
/// and otherwise as a response resource whose data is that whole envelope
/// (`RNS/Link.py:849-852`); the requester unpacks `data[0]`/`data[1]` from
/// it (`RNS/Link.py:907-912`). Neither path compresses a packet; the
/// resource honours the registration's `auto_compress`.
///
/// `Resource` is the file-response form (`RNS/Link.py:839-846`): always a
/// resource, with `data` as the raw resource payload, not an envelope, and
/// `metadata` packed as msgpack ahead of it. Python's requester reads a
/// resource as a file response only when it carries metadata
/// (`RNS/Link.py:902-912`), so a `Resource { metadata: None }` answer is
/// unpacked as an envelope there and is readable only by a peer that
/// expects raw bytes, as Python's own `(file_handle,)` tuple is.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum RequestResponse {
    Value(rmpv::Value),
    Bytes(Vec<u8>),
    Resource { data: Vec<u8>, metadata: Option<rmpv::Value> },
}

/// The `response_generator` of `RNS.Destination.register_request_handler`
/// (`RNS/Destination.py:381-398`).
///
/// Every `Fn(InboundRequest) -> impl Future<Output = Option<RequestResponse>>`
/// whose future is `Send + 'static` is a handler through the blanket impl
/// below; the `'static` is what lets the boxed future coerce to the `'_`
/// trait object.
///
/// Each request runs on its own task with no concurrency cap, as Python
/// runs a thread per request; a handler that needs a bound holds its own
/// `Semaphore`.
pub trait RequestHandler: Send + Sync {
    /// The `response_generator` call (`RNS/Link.py:831`); `None` sends no
    /// response (`RNS/Link.py:844`).
    fn handle(
        &self,
        request: InboundRequest,
    ) -> Pin<Box<dyn Future<Output = Option<RequestResponse>> + Send + '_>>;
}

impl<F, Fut> RequestHandler for F
where
    F: Fn(InboundRequest) -> Fut + Send + Sync,
    Fut: Future<Output = Option<RequestResponse>> + Send + 'static,
{
    fn handle(
        &self,
        request: InboundRequest,
    ) -> Pin<Box<dyn Future<Output = Option<RequestResponse>> + Send + '_>> {
        Box::pin((self)(request))
    }
}

/// One row of a destination's handler table
/// (`RNS/Destination.py:397-398`: `[path, response_generator, allow,
/// allowed_list, auto_compress]`).
pub(crate) struct RegisteredHandler {
    pub(crate) path: String,
    pub(crate) handler: Arc<dyn RequestHandler>,
    pub(crate) allow: RequestAllow,
    pub(crate) auto_compress: bool,
}
