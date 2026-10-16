//! Wire frames of the link request layer, byte-compatible with
//! `RNS.Link.request` and `RNS.Link.handle_request`.
//!
//! A request travels as msgpack `[requested_at, path_hash, data]`
//! (`RNS/Link.py:486-488`), a response as `[request_id, body]`
//! (`RNS/Link.py:848`). [`RequestFrame`] and [`ResponseFrame`] encode and
//! decode those arrays; [`PathHash`] and [`RequestId`] are the two truncated
//! hashes they carry. [`RequestHandler`], [`RequestAllow`], [`InboundRequest`]
//! and [`RequestResponse`] are the responder API a destination registers
//! handlers with; [`RequestOptions`], [`RequestReceipt`], [`RequestStatus`],
//! [`RequestError`] and [`RequestResponseData`] are the requester's side of
//! `Transport::request`.
//!
//! # Decoding
//!
//! The crate reports every decode failure at a link boundary as
//! [`RnsError::PacketError`](crate::error::RnsError::PacketError), which
//! cannot tell a nesting refusal from a length refusal. The decoders here
//! return [`FrameDecodeError`] instead, so tests and fuzz harnesses can pin
//! each rule on its own, and the error collapses to `PacketError` through
//! `From` where `Link` consumes it.
//!
//! Before any value is built, the decoder walks the input once without
//! allocating: it follows msgpack markers and length prefixes over the raw
//! slice, refusing container nesting (arrays and maps, the frame's own array
//! counting as level one) deeper than [`MAX_DEPTH`], a `bin`, `str` or `ext`
//! length larger than the bytes left, the reserved marker `0xc1`, a value
//! that ends before its input does, and bytes after the frame. Only a slice
//! that passes is handed to `rmpv::decode::read_value`. `rmpv` on its own
//! caps `bin` and `str` preallocation at 64 KiB, grows arrays by push, and
//! offers `read_value_with_max_depth`, but it counts `str`, `bin` and `ext`
//! as depth levels and still allocates before a short input is noticed; the
//! walk is what makes a refusal allocation-free and makes the depth rule
//! mean containers.
#![deny(clippy::await_holding_invalid_type)]

mod allow;
mod dispatch;
mod frame;
mod handler;
mod id;
mod inbound;
mod receipt;
mod receipt_pending;
#[cfg(test)]
mod receipt_tests;
#[cfg(test)]
mod tests;

pub use allow::RequestAllow;
pub(crate) use dispatch::{DispatchForm, DispatchItem, REQUEST_DISPATCH_CAPACITY};
pub use frame::{FrameDecodeError, RequestFrame, ResponseFrame, MAX_DEPTH};
pub(crate) use handler::RegisteredHandler;
pub use handler::{RequestHandler, RequestResponse};
pub use id::{PathHash, RequestId};
pub use inbound::InboundRequest;
pub(crate) use receipt::ResponseArrival;
pub use receipt::{
    RequestError, RequestOptions, RequestReceipt, RequestResponseData, RequestStatus,
};
pub(crate) use receipt_pending::PendingRequest;
