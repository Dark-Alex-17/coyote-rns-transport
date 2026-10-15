//! Wire frames of the link request layer, byte-compatible with
//! `RNS.Link.request` and `RNS.Link.handle_request`.
//!
//! A request travels as msgpack `[requested_at, path_hash, data]`
//! (`RNS/Link.py:486-488`), a response as `[request_id, body]`
//! (`RNS/Link.py:848`). [`RequestFrame`] and [`ResponseFrame`] encode and
//! decode those arrays; [`PathHash`] and [`RequestId`] are the two truncated
//! hashes they carry. Dispatch, handlers and receipts are not part of this
//! module.
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

mod frame;
mod id;
#[cfg(test)]
mod tests;

pub use frame::{FrameDecodeError, RequestFrame, ResponseFrame, MAX_DEPTH};
pub use id::{PathHash, RequestId};
