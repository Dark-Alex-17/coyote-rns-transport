use std::io::Cursor;

use rmp::Marker;
use rmpv::Value;

use crate::error::RnsError;
use crate::hash::ADDRESS_HASH_SIZE;

use super::id::{PathHash, RequestId};

/// Deepest container nesting a frame may carry. Arrays and maps count, the
/// frame's own outer array is level one. Fork-only: Python decodes with
/// `umsgpack.unpackb` and sets no depth limit (`RNS/Link.py:1003`, `:1013`);
/// a frame deeper than the interpreter's recursion limit raises inside
/// `unpackb` and is dropped at `RNS/Link.py:1007`.
pub const MAX_DEPTH: usize = 128;

/// Why a byte string is not a request or response frame.
///
/// Carries no heap data, so refusing an input allocates nothing. Converts
/// into [`RnsError::PacketError`], the crate's decode failure at link
/// boundaries. Fork-only: Python hands the bytes straight to
/// `umsgpack.unpackb` with no shape or depth pre-walk (`RNS/Link.py:1003`,
/// `:888` request side; `:1013`, `:909` response side).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameDecodeError {
    /// Fork-only: the input ends inside a value.
    #[error("input ends inside a msgpack value")]
    Truncated,
    /// Fork-only: bytes follow the frame.
    #[error("bytes follow the frame")]
    TrailingBytes,
    /// Fork-only: arrays and maps nest deeper than [`MAX_DEPTH`].
    #[error("containers nest deeper than {MAX_DEPTH} levels")]
    DepthExceeded,
    /// Fork-only: a `bin`, `str` or `ext` length prefix claims more bytes
    /// than remain.
    #[error("length prefix larger than the remaining input")]
    LengthExceedsInput,
    /// Fork-only: the reserved marker `0xc1` appears.
    #[error("reserved msgpack marker 0xc1")]
    ReservedMarker,
    /// Fork-only: the value reader failed on an input the walk accepted.
    /// Unreachable while the walk mirrors `rmpv`'s reader; kept so a future
    /// `rmpv` change surfaces as an error instead of a panic.
    #[error("msgpack value could not be read")]
    Malformed,
    /// Fork-only: the frame is not an array.
    #[error("frame is not an array")]
    NotAnArray,
    /// Fork-only: the array has the wrong number of elements.
    #[error("frame has {found} elements, expected {expected}")]
    WrongArity {
        /// Elements the frame shape requires.
        expected: usize,
        /// Elements the input carries.
        found: usize,
    },
    /// Fork-only: `requested_at` is not a `float64` (marker `0xcb`).
    #[error("requested_at is not a float64")]
    RequestedAtNotF64,
    /// Fork-only: the path hash or request id is not a 16-byte `bin`.
    #[error("hash is not a 16-byte bin")]
    HashNotBin16,
}

impl From<FrameDecodeError> for RnsError {
    fn from(_: FrameDecodeError) -> Self {
        RnsError::PacketError
    }
}

/// `[requested_at, path_hash, data]`, the request `RNS.Link.request` packs
/// (`RNS/Link.py:486-488`) and `RNS.Link.handle_request` reads back by index
/// (`RNS/Link.py:806-808`).
#[derive(Debug, Clone, PartialEq)]
pub struct RequestFrame {
    requested_at: f64,
    path_hash: PathHash,
    data: Value,
}

impl RequestFrame {
    /// A request for the path hashing to `path_hash`, carrying `data`: the
    /// `[time.time(), request_path_hash, data]` list (`RNS/Link.py:487`).
    pub fn new(requested_at: f64, path_hash: PathHash, data: Value) -> Self {
        Self { requested_at, path_hash, data }
    }

    /// `time.time()` on the requesting side, seconds since the epoch
    /// (`RNS/Link.py:487`); the responder reads it as `unpacked_request[0]`
    /// (`:806`).
    pub fn requested_at(&self) -> f64 {
        self.requested_at
    }

    /// `truncated_hash` of the requested path (`RNS/Link.py:486`).
    pub fn path_hash(&self) -> PathHash {
        self.path_hash
    }

    /// The request body in its own msgpack type: `handle_request` passes
    /// `unpacked_request[2]` (`RNS/Link.py:808`) to the handler unchanged.
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// The request body, `unpacked_request[2]` (`RNS/Link.py:808`), moved out
    /// of the frame.
    pub fn into_data(self) -> Value {
        self.data
    }

    /// The bytes `umsgpack.packb([time.time(), request_path_hash, data])`
    /// produces: `requested_at` is always `float64` (marker `0xcb`), as
    /// `umsgpack._pack_float` writes it at double precision
    /// (`RNS/vendor/umsgpack.py:320-324,989`).
    pub fn encode(&self) -> Vec<u8> {
        write(&Value::Array(vec![
            Value::F64(self.requested_at),
            Value::Binary(self.path_hash.to_bytes().to_vec()),
            self.data.clone(),
        ]))
    }

    /// Reads a request frame. Exactly three elements; `requested_at` must be
    /// `float64` (a `float32` or an integer is refused) and the path hash a
    /// 16-byte `bin`. Fork-only: Python hands the bytes to
    /// `umsgpack.unpackb` with no shape check (`RNS/Link.py:1003`).
    pub fn decode(input: &[u8]) -> Result<Self, FrameDecodeError> {
        let [requested_at, path_hash, data] = read_array(input)?;
        let Value::F64(requested_at) = requested_at else {
            return Err(FrameDecodeError::RequestedAtNotF64);
        };
        let path_hash = PathHash::new(bin16(&path_hash)?);
        Ok(Self { requested_at, path_hash, data })
    }
}

/// `[request_id, response]`, the envelope `RNS.Link.handle_request` packs
/// (`RNS/Link.py:848`) and the requester reads back by index
/// (`RNS/Link.py:1014-1015`).
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseFrame {
    request_id: RequestId,
    body: Value,
}

impl ResponseFrame {
    /// A response to the request identified by `request_id`, carrying `body`:
    /// the `[request_id, response]` list (`RNS/Link.py:848`).
    pub fn new(request_id: RequestId, body: Value) -> Self {
        Self { request_id, body }
    }

    /// The id of the request this frame answers, `unpacked_response[0]`
    /// (`RNS/Link.py:1014`).
    pub fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// The handler's return value in its own msgpack type, as
    /// `handle_request` packs it; a handler that returns `None` sends no
    /// frame at all (`RNS/Link.py:844`).
    pub fn body(&self) -> &Value {
        &self.body
    }

    /// The handler's return value, `unpacked_response[1]` (`RNS/Link.py:1015`),
    /// moved out of the frame.
    pub fn into_body(self) -> Value {
        self.body
    }

    /// The bytes `umsgpack.packb([request_id, response])` produces
    /// (`RNS/Link.py:848`).
    pub fn encode(&self) -> Vec<u8> {
        write(&Value::Array(vec![
            Value::Binary(self.request_id.to_bytes().to_vec()),
            self.body.clone(),
        ]))
    }

    /// Reads a response frame. Exactly two elements; the request id must be
    /// a 16-byte `bin`. Fork-only: Python hands the bytes to
    /// `umsgpack.unpackb` with no shape check (`RNS/Link.py:1013`).
    pub fn decode(input: &[u8]) -> Result<Self, FrameDecodeError> {
        let [request_id, body] = read_array(input)?;
        let request_id = RequestId::new(bin16(&request_id)?);
        Ok(Self { request_id, body })
    }
}

fn write(value: &Value) -> Vec<u8> {
    let mut packed = Vec::new();
    rmpv::encode::write_value(&mut packed, value).expect("writing msgpack into a Vec cannot fail");
    packed
}

fn read_array<const N: usize>(input: &[u8]) -> Result<[Value; N], FrameDecodeError> {
    validate(input)?;
    let value = rmpv::decode::read_value(&mut Cursor::new(input))
        .map_err(|_| FrameDecodeError::Malformed)?;
    let Value::Array(values) = value else {
        return Err(FrameDecodeError::NotAnArray);
    };
    let found = values.len();
    <[Value; N]>::try_from(values).map_err(|_| FrameDecodeError::WrongArity { expected: N, found })
}

fn bin16(value: &Value) -> Result<[u8; ADDRESS_HASH_SIZE], FrameDecodeError> {
    match value {
        Value::Binary(bytes) => <[u8; ADDRESS_HASH_SIZE]>::try_from(bytes.as_slice())
            .map_err(|_| FrameDecodeError::HashNotBin16),
        _ => Err(FrameDecodeError::HashNotBin16),
    }
}

/// Walks exactly one msgpack value over `input` without building it, so a
/// refusal costs no allocation. See the module doc for the rules.
pub(super) fn validate(input: &[u8]) -> Result<(), FrameDecodeError> {
    let rest = walk_value(input, 0)?;
    if rest.is_empty() {
        Ok(())
    } else {
        Err(FrameDecodeError::TrailingBytes)
    }
}

/// Walks the value at the head of `input`, nested `depth` containers deep,
/// and returns the bytes after it.
fn walk_value(input: &[u8], depth: usize) -> Result<&[u8], FrameDecodeError> {
    let (&marker, rest) = input.split_first().ok_or(FrameDecodeError::Truncated)?;
    match Marker::from_u8(marker) {
        Marker::FixPos(_) | Marker::FixNeg(_) | Marker::Null | Marker::True | Marker::False => {
            Ok(rest)
        }
        Marker::Reserved => Err(FrameDecodeError::ReservedMarker),
        Marker::U8 | Marker::I8 => scalar(rest, 1),
        Marker::U16 | Marker::I16 => scalar(rest, 2),
        Marker::U32 | Marker::I32 | Marker::F32 => scalar(rest, 4),
        Marker::U64 | Marker::I64 | Marker::F64 => scalar(rest, 8),
        Marker::FixStr(len) => skip(rest, len as usize),
        Marker::Str8 | Marker::Bin8 => {
            let (len, rest) = length_u8(rest)?;
            skip(rest, len)
        }
        Marker::Str16 | Marker::Bin16 => {
            let (len, rest) = length_u16(rest)?;
            skip(rest, len)
        }
        Marker::Str32 | Marker::Bin32 => {
            let (len, rest) = length_u32(rest)?;
            skip(rest, len)
        }
        Marker::FixExt1 => skip_ext(rest, 1),
        Marker::FixExt2 => skip_ext(rest, 2),
        Marker::FixExt4 => skip_ext(rest, 4),
        Marker::FixExt8 => skip_ext(rest, 8),
        Marker::FixExt16 => skip_ext(rest, 16),
        Marker::Ext8 => {
            let (len, rest) = length_u8(rest)?;
            skip_ext(rest, len)
        }
        Marker::Ext16 => {
            let (len, rest) = length_u16(rest)?;
            skip_ext(rest, len)
        }
        Marker::Ext32 => {
            let (len, rest) = length_u32(rest)?;
            skip_ext(rest, len)
        }
        Marker::FixArray(count) => walk_entries(rest, count as usize, 1, depth),
        Marker::Array16 => {
            let (count, rest) = length_u16(rest)?;
            walk_entries(rest, count, 1, depth)
        }
        Marker::Array32 => {
            let (count, rest) = length_u32(rest)?;
            walk_entries(rest, count, 1, depth)
        }
        Marker::FixMap(count) => walk_entries(rest, count as usize, 2, depth),
        Marker::Map16 => {
            let (count, rest) = length_u16(rest)?;
            walk_entries(rest, count, 2, depth)
        }
        Marker::Map32 => {
            let (count, rest) = length_u32(rest)?;
            walk_entries(rest, count, 2, depth)
        }
    }
}

/// Walks a container of `count` entries of `values_per_entry` values each
/// (one for an array, key and value for a map) at nesting `depth + 1`.
fn walk_entries(
    mut input: &[u8],
    count: usize,
    values_per_entry: usize,
    depth: usize,
) -> Result<&[u8], FrameDecodeError> {
    let depth = depth + 1;
    if depth > MAX_DEPTH {
        return Err(FrameDecodeError::DepthExceeded);
    }
    // Every value is at least one byte, so a count the input cannot hold is
    // refused before the walk starts.
    if count > input.len() / values_per_entry {
        return Err(FrameDecodeError::Truncated);
    }
    for _ in 0..count {
        for _ in 0..values_per_entry {
            input = walk_value(input, depth)?;
        }
    }
    Ok(input)
}

/// Skips a fixed-width number body.
fn scalar(input: &[u8], width: usize) -> Result<&[u8], FrameDecodeError> {
    input.get(width..).ok_or(FrameDecodeError::Truncated)
}

/// Skips a body whose length a prefix declared.
fn skip(input: &[u8], len: usize) -> Result<&[u8], FrameDecodeError> {
    input.get(len..).ok_or(FrameDecodeError::LengthExceedsInput)
}

/// Skips an `ext` body: the type byte, then `len` payload bytes.
fn skip_ext(input: &[u8], len: usize) -> Result<&[u8], FrameDecodeError> {
    let payload = input.get(1..).ok_or(FrameDecodeError::Truncated)?;
    skip(payload, len)
}

fn length_u8(input: &[u8]) -> Result<(usize, &[u8]), FrameDecodeError> {
    let (&len, rest) = input.split_first().ok_or(FrameDecodeError::Truncated)?;
    Ok((len as usize, rest))
}

fn length_u16(input: &[u8]) -> Result<(usize, &[u8]), FrameDecodeError> {
    let (len, rest) = input.split_first_chunk::<2>().ok_or(FrameDecodeError::Truncated)?;
    Ok((u16::from_be_bytes(*len) as usize, rest))
}

fn length_u32(input: &[u8]) -> Result<(usize, &[u8]), FrameDecodeError> {
    let (len, rest) = input.split_first_chunk::<4>().ok_or(FrameDecodeError::Truncated)?;
    let len = usize::try_from(u32::from_be_bytes(*len))
        .map_err(|_| FrameDecodeError::LengthExceedsInput)?;
    Ok((len, rest))
}
