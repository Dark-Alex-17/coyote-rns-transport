//! Black-box usage of the link request wire frames from outside the crate:
//! `rns_transport::destination::request::{RequestFrame, ResponseFrame,
//! RequestId, PathHash, FrameDecodeError, MAX_DEPTH}`.
//!
//! Every byte vector below was produced by RNS 1.5.2's own `umsgpack` and
//! `RNS.Identity.truncated_hash` (`RNS/Link.py:486-488`, `:504`, `:848`,
//! `RNS/Packet.py:355`, `:361-365`), independently of the Rust encoder:
//!
//! ```text
//! ph = RNS.Identity.truncated_hash(path.encode("utf-8"))
//! umsgpack.packb([ts, ph, data]).hex()
//! umsgpack.packb([request_id, body]).hex()
//! ```

use std::time::{Duration, Instant};

use rns_transport::destination::link::{unpack_response_envelope, Link};
use rns_transport::destination::request::{
    FrameDecodeError, PathHash, RequestFrame, RequestId, ResponseFrame, MAX_DEPTH,
};
use rns_transport::hash::Hash;
use rns_transport::rmpv::Value;

const PATH: &str = "example/path";
const PATH_HASH_HEX: &str = "a2815deccd7e741285784132e2e477dc";
const TS: f64 = 1700000000.5;

fn hex(s: &str) -> Vec<u8> {
    hex::decode(s).expect("hex fixture")
}

fn hex16(s: &str) -> [u8; 16] {
    <[u8; 16]>::try_from(hex(s).as_slice()).expect("16-byte fixture")
}

/// `(data, umsgpack.packb([1700000000.5, ph, data]).hex(), truncated_hash(packed).hex())`
fn python_request_vectors() -> Vec<(&'static str, Value, &'static str, &'static str)> {
    vec![
        (
            "nil",
            Value::Nil,
            "93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dcc0",
            "8a2adedd7b2141af73c27f5ef7084b54",
        ),
        (
            "map",
            Value::Map(vec![(Value::from("k"), Value::from(1))]),
            "93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc81a16b01",
            "2b3e92d35ae6c727f008745562621b64",
        ),
        (
            "array",
            Value::Array(vec![Value::from(1), Value::from("two"), Value::Binary(vec![3])]),
            "93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc9301a374776fc40103",
            "14cf005319ef12be1c471f61733efa1c",
        ),
        (
            "bytes",
            Value::Binary(vec![0, 1, 2]),
            "93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dcc403000102",
            "85d5223948cdc398fd260cace4b5cf98",
        ),
        (
            "str",
            Value::from("hello"),
            "93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dca568656c6c6f",
            "0e95ac0455c2253d91c0a76a2cdfc32a",
        ),
    ]
}

#[test]
fn path_hash_matches_python_truncated_hash() {
    let hash = PathHash::of(PATH);
    assert_eq!(hash.as_bytes(), &hex16(PATH_HASH_HEX));
    assert_eq!(hash.to_bytes(), hex16(PATH_HASH_HEX));
    assert_eq!(hash.to_string(), PATH_HASH_HEX);
    assert_eq!(PathHash::new(hex16(PATH_HASH_HEX)), hash);
    assert_eq!(PathHash::from(hex16(PATH_HASH_HEX)), hash);
    // RNS.Identity.truncated_hash(b"x")
    assert_eq!(PathHash::of("x").to_string(), "2d711642b726b04401627ca9fbac32f5");
}

#[test]
fn request_id_resource_form_is_truncated_hash_of_packed_request() {
    for (name, _, packed_hex, id_hex) in python_request_vectors() {
        let id = RequestId::of_packed_request(&hex(packed_hex));
        assert_eq!(id.to_string(), id_hex, "resource-form id for {name}");
        assert_eq!(id, RequestId::new(hex16(id_hex)));
        assert_eq!(id, RequestId::from(hex16(id_hex)));
        assert_eq!(id.as_bytes(), &hex16(id_hex));
    }
}

#[test]
fn request_id_packet_form_is_truncated_hash_of_packet_hashable_part() {
    // HEADER_1 raw packet: flags 0x02, hops 0x05, 16-byte destination, context
    // 0x0b, data aa bb cc. Hashable part is bytes([raw[0] & 0x0f]) + raw[2:]
    // (RNS/Packet.py:361-365); Python truncated_hash of it is below.
    let hashable = hex("02000102030405060708090a0b0c0d0e0f0baabbcc");
    let id = RequestId::from_packet_hash(&Hash::new_from_slice(&hashable));
    assert_eq!(id.to_string(), "fdb3e183ed60776806bdf1a3491edbd2");
    // The packet form hashes header + destination + context + payload; the
    // resource form hashes the packed request alone, so the same payload
    // gets a different id on each path.
    assert_ne!(id, RequestId::of_packed_request(&hex("aabbcc")));
}

#[test]
fn request_frame_encodes_byte_for_byte_like_umsgpack() {
    for (name, data, packed_hex, _) in python_request_vectors() {
        let frame = RequestFrame::new(TS, PathHash::of(PATH), data);
        assert_eq!(frame.encode(), hex(packed_hex), "encode({name})");
    }
}

#[test]
fn request_frame_decodes_python_bytes_and_round_trips() {
    for (name, data, packed_hex, _) in python_request_vectors() {
        let bytes = hex(packed_hex);
        let frame = RequestFrame::decode(&bytes).unwrap_or_else(|e| panic!("decode({name}): {e}"));
        assert_eq!(frame.requested_at(), TS, "{name}");
        assert_eq!(frame.path_hash(), PathHash::of(PATH), "{name}");
        assert_eq!(frame.data(), &data, "{name}");
        assert_eq!(frame.encode(), bytes, "re-encode({name})");
        assert_eq!(frame.clone().into_data(), data, "{name}");
    }
}

#[test]
fn request_frame_timestamp_is_always_float64_marker_0xcb() {
    // umsgpack.packb([1700000000.0, ph, None]) -- an integer-valued float
    // still packs as float64 0xcb, never as a msgpack int.
    let frame = RequestFrame::new(1700000000.0, PathHash::of(PATH), Value::Nil);
    let bytes = frame.encode();
    assert_eq!(bytes[1], 0xcb);
    assert_eq!(hex("93cb41d954fc40000000c410a2815deccd7e741285784132e2e477dcc0"), bytes);
    // Same for 0.0 and a 1-char path: umsgpack.packb([0.0, truncated_hash(b"x"), None])
    let zero = RequestFrame::new(0.0, PathHash::of("x"), Value::Nil).encode();
    assert_eq!(hex("93cb0000000000000000c4102d711642b726b04401627ca9fbac32f5c0"), zero);
    for ts in [0.0, 1.0, -1.0, 1e-9, 1.5e12, f64::MAX] {
        assert_eq!(
            RequestFrame::new(ts, PathHash::of(PATH), Value::Nil).encode()[1],
            0xcb,
            "ts={ts}"
        );
    }
}

#[test]
fn request_frame_agrees_with_link_request_payload() {
    for (name, data, _, _) in python_request_vectors() {
        let payload = Link::request_payload(PATH, data.clone()).expect("request_payload");
        let frame = RequestFrame::decode(&payload.packed).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(frame.path_hash().to_bytes(), payload.path_hash, "{name}");
        assert_eq!(frame.data(), &data, "{name}");
        // Re-encoding with the timestamp the payload carried reproduces it exactly.
        let again = RequestFrame::new(frame.requested_at(), frame.path_hash(), data.clone());
        assert_eq!(again.encode(), payload.packed, "{name}");
        assert_eq!(
            RequestId::of_packed_request(&payload.packed).to_bytes(),
            payload.resource_request_id,
            "{name}"
        );
        assert_eq!(payload.packed[1], 0xcb, "{name}");
    }
}

#[test]
fn request_frame_refuses_wrong_shapes() {
    let good = hex("93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dcc0");
    // Not an array: a map with 3 entries, or a bare int.
    assert_eq!(RequestFrame::decode(&[0x01]), Err(FrameDecodeError::NotAnArray));
    // Two elements [ts, ph].
    let two = hex("92cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc");
    assert_eq!(
        RequestFrame::decode(&two),
        Err(FrameDecodeError::WrongArity { expected: 3, found: 2 })
    );
    // Four elements [ts, ph, nil, nil].
    let four = hex("94cb41d954fc40200000c410a2815deccd7e741285784132e2e477dcc0c0");
    assert_eq!(
        RequestFrame::decode(&four),
        Err(FrameDecodeError::WrongArity { expected: 3, found: 4 })
    );
    // Timestamp as int (umsgpack would never emit this).
    let int_ts = hex("9301c410a2815deccd7e741285784132e2e477dcc0");
    assert_eq!(RequestFrame::decode(&int_ts), Err(FrameDecodeError::RequestedAtNotF64));
    // Timestamp as float32.
    let f32_ts = hex("93ca3f800000c410a2815deccd7e741285784132e2e477dcc0");
    assert_eq!(RequestFrame::decode(&f32_ts), Err(FrameDecodeError::RequestedAtNotF64));
    // Path hash 15 bytes.
    let short_hash = hex("93cb41d954fc40200000c40fa2815deccd7e741285784132e2e477c0");
    assert_eq!(RequestFrame::decode(&short_hash), Err(FrameDecodeError::HashNotBin16));
    // Path hash as str16 instead of bin16.
    let str_hash = hex("93cb41d954fc40200000b0a2815deccd7e741285784132e2e477dcc0");
    assert_eq!(RequestFrame::decode(&str_hash), Err(FrameDecodeError::HashNotBin16));
    // Trailing byte after an otherwise valid frame.
    let mut trailing = good.clone();
    trailing.push(0xc0);
    assert_eq!(RequestFrame::decode(&trailing), Err(FrameDecodeError::TrailingBytes));
    // Cut inside the fixed-width f64: the input ends inside a value.
    assert_eq!(RequestFrame::decode(&good[..5]), Err(FrameDecodeError::Truncated));
    // Cut inside the bin 16: its length prefix now claims more than remains.
    assert_eq!(RequestFrame::decode(&good[..20]), Err(FrameDecodeError::LengthExceedsInput));
    // Empty input.
    assert!(RequestFrame::decode(&[]).is_err());
    // The good frame still decodes.
    assert!(RequestFrame::decode(&good).is_ok());
}

fn response_id() -> RequestId {
    RequestId::new(hex16("000102030405060708090a0b0c0d0e0f"))
}

/// `(body, umsgpack.packb([bytes(range(16)), body]).hex())`
fn python_response_vectors() -> Vec<(&'static str, Value, &'static str)> {
    vec![
        ("nil", Value::Nil, "92c410000102030405060708090a0b0c0d0e0fc0"),
        (
            "map",
            Value::Map(vec![(
                Value::from("a"),
                Value::Array(vec![Value::from(1), Value::from(2)]),
            )]),
            "92c410000102030405060708090a0b0c0d0e0f81a161920102",
        ),
        (
            "bytes",
            Value::Binary(vec![0xff; 4]),
            "92c410000102030405060708090a0b0c0d0e0fc404ffffffff",
        ),
        ("int", Value::from(7), "92c410000102030405060708090a0b0c0d0e0f07"),
    ]
}

#[test]
fn response_frame_encodes_byte_for_byte_like_umsgpack() {
    for (name, body, packed_hex) in python_response_vectors() {
        let frame = ResponseFrame::new(response_id(), body);
        assert_eq!(frame.encode(), hex(packed_hex), "encode({name})");
    }
}

#[test]
fn response_frame_decodes_python_bytes_and_round_trips() {
    for (name, body, packed_hex) in python_response_vectors() {
        let bytes = hex(packed_hex);
        let frame = ResponseFrame::decode(&bytes).unwrap_or_else(|e| panic!("decode({name}): {e}"));
        assert_eq!(frame.request_id(), response_id(), "{name}");
        assert_eq!(frame.body(), &body, "{name}");
        assert_eq!(frame.encode(), bytes, "{name}");
        assert_eq!(frame.clone().into_body(), body, "{name}");
    }
}

#[test]
fn response_frame_agrees_with_unpack_response_envelope() {
    for (name, body, packed_hex) in python_response_vectors() {
        let bytes = hex(packed_hex);
        let (id, value) = unpack_response_envelope(&bytes).expect("envelope");
        let frame = ResponseFrame::decode(&bytes).expect("frame");
        assert_eq!(frame.request_id().to_bytes(), id, "{name}");
        assert_eq!(frame.body(), &value, "{name}");
        assert_eq!(value, body, "{name}");
    }
}

#[test]
fn response_frame_refuses_wrong_shapes() {
    let good = hex("92c410000102030405060708090a0b0c0d0e0fc0");
    assert_eq!(ResponseFrame::decode(&[0xc0]), Err(FrameDecodeError::NotAnArray));
    assert_eq!(
        ResponseFrame::decode(&hex("91c410000102030405060708090a0b0c0d0e0f")),
        Err(FrameDecodeError::WrongArity { expected: 2, found: 1 })
    );
    assert_eq!(
        ResponseFrame::decode(&hex("93c410000102030405060708090a0b0c0d0e0fc0c0")),
        Err(FrameDecodeError::WrongArity { expected: 2, found: 3 })
    );
    // 17-byte id.
    assert_eq!(
        ResponseFrame::decode(&hex("92c411000102030405060708090a0b0c0d0e0f10c0")),
        Err(FrameDecodeError::HashNotBin16)
    );
    // id as array.
    assert_eq!(ResponseFrame::decode(&hex("9290c0")), Err(FrameDecodeError::HashNotBin16));
    let mut trailing = good.clone();
    trailing.extend_from_slice(&[0x00, 0x01]);
    assert_eq!(ResponseFrame::decode(&trailing), Err(FrameDecodeError::TrailingBytes));
    assert_eq!(ResponseFrame::decode(&good[..good.len() - 1]), Err(FrameDecodeError::Truncated));
    assert!(ResponseFrame::decode(&good).is_ok());
}

/// `[ts, ph, data]` where `data` is `depth` nested single-element arrays
/// around nil.
fn request_with_nested_data(depth: usize) -> Vec<u8> {
    let mut bytes = hex("93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc");
    bytes.extend(std::iter::repeat_n(0x91, depth));
    bytes.push(0xc0);
    bytes
}

#[test]
fn max_depth_is_128_and_counts_containers_from_the_outer_array() {
    assert_eq!(MAX_DEPTH, 128);
    // Outer array is level 1, so data may nest MAX_DEPTH - 1 more levels.
    let at_limit = request_with_nested_data(MAX_DEPTH - 1);
    let frame = RequestFrame::decode(&at_limit).expect("depth == MAX_DEPTH decodes");
    assert_eq!(frame.encode(), at_limit);
    let over = request_with_nested_data(MAX_DEPTH);
    assert_eq!(RequestFrame::decode(&over), Err(FrameDecodeError::DepthExceeded));
    // Maps count too: data = {nil: {nil: ...}} nested MAX_DEPTH deep.
    let mut maps = hex("93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc");
    for _ in 0..MAX_DEPTH {
        maps.extend_from_slice(&[0x81, 0xc0]);
    }
    maps.push(0xc0);
    assert_eq!(RequestFrame::decode(&maps), Err(FrameDecodeError::DepthExceeded));
    // bin/str do not count as nesting levels: a 1-byte bin inside the
    // deepest allowed array is still accepted.
    let mut deep_bin = hex("93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc");
    deep_bin.extend(std::iter::repeat_n(0x91, MAX_DEPTH - 1));
    deep_bin.extend_from_slice(&[0xc4, 0x01, 0x2a]);
    assert!(RequestFrame::decode(&deep_bin).is_ok());
}

#[test]
fn pathological_nesting_does_not_overflow_the_stack() {
    // 1 MiB of nested array markers with no terminator. Must return Err, not
    // crash, and must do so without building a value tree.
    let huge = request_with_nested_data(1 << 20);
    let started = Instant::now();
    let err = RequestFrame::decode(&huge).expect_err("refused");
    assert_eq!(err, FrameDecodeError::DepthExceeded);
    assert!(started.elapsed() < Duration::from_secs(1), "took {:?}", started.elapsed());
    let err = ResponseFrame::decode(&huge).expect_err("refused");
    assert_eq!(err, FrameDecodeError::DepthExceeded);
}

#[test]
fn oversized_length_prefixes_are_refused_fast_without_allocating() {
    // Each input is at most a few dozen bytes but claims a 2 GiB (or 4 GiB)
    // payload. A decoder that trusted the prefix would try to allocate it.
    let prefix = hex("93cb41d954fc40200000");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("bin32 0x80000000 as path hash", [prefix.clone(), hex("c680000000")].concat()),
        ("bin32 0xffffffff as path hash", [prefix.clone(), hex("c6ffffffff")].concat()),
        (
            "str32 0x80000000 as data",
            [prefix.clone(), hex("c410a2815deccd7e741285784132e2e477dc"), hex("db80000000")]
                .concat(),
        ),
        (
            "ext32 0x80000000 as data",
            [prefix.clone(), hex("c410a2815deccd7e741285784132e2e477dc"), hex("c98000000001")]
                .concat(),
        ),
        (
            "array32 0x80000000 as data",
            [prefix.clone(), hex("c410a2815deccd7e741285784132e2e477dc"), hex("dd80000000")]
                .concat(),
        ),
        (
            "map32 0x80000000 as data",
            [prefix.clone(), hex("c410a2815deccd7e741285784132e2e477dc"), hex("df80000000")]
                .concat(),
        ),
        ("bin16 0xffff as path hash", [prefix.clone(), hex("c5ffff")].concat()),
        ("response bin32 0x80000000 id", hex("92c680000000")),
        ("response str32 body", hex("92c410000102030405060708090a0b0c0d0e0fdbffffffff")),
        ("response array32 body", hex("92c410000102030405060708090a0b0c0d0e0fddffffffff")),
        ("ten bytes bin32 0x80000000", hex("92c68000000000000000")),
    ];
    for (name, input) in cases {
        assert!(input.len() <= 48, "{name}: fixture is {} bytes", input.len());
        let started = Instant::now();
        let req = RequestFrame::decode(&input);
        let resp = ResponseFrame::decode(&input);
        let elapsed = started.elapsed();
        assert!(req.is_err(), "{name}: RequestFrame accepted");
        assert!(resp.is_err(), "{name}: ResponseFrame accepted");
        assert!(elapsed < Duration::from_millis(100), "{name}: took {elapsed:?}");
        for err in [req.unwrap_err(), resp.unwrap_err()] {
            assert!(
                matches!(err, FrameDecodeError::LengthExceedsInput | FrameDecodeError::Truncated),
                "{name}: unexpected error {err:?}"
            );
        }
    }
}

#[test]
fn reserved_marker_is_refused() {
    let mut bytes = hex("93cb41d954fc40200000c410a2815deccd7e741285784132e2e477dc");
    bytes.push(0xc1);
    assert_eq!(RequestFrame::decode(&bytes), Err(FrameDecodeError::ReservedMarker));
}

#[test]
fn decode_error_converts_to_packet_error_and_is_copy() {
    use rns_transport::error::RnsError;
    let err: FrameDecodeError = RequestFrame::decode(&[0x01]).unwrap_err();
    let copy = err;
    assert!(matches!(RnsError::from(err), RnsError::PacketError));
    assert_eq!(copy.to_string(), "frame is not an array");
    assert_eq!(
        FrameDecodeError::DepthExceeded.to_string(),
        "containers nest deeper than 128 levels"
    );
}
