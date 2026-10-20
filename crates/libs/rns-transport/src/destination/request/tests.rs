use std::time::{Duration, Instant};

use rmpv::Value;

use super::frame::validate;
use super::{FrameDecodeError, PathHash, RequestFrame, RequestId, ResponseFrame, MAX_DEPTH};
use crate::destination::link::{unpack_response_envelope, Link};
use crate::error::RnsError;
use crate::hash::{Hash, ADDRESS_HASH_SIZE};
use crate::resource::ResourceAdvertisement;

/// Packed by RNS 1.5.2 (ea98db4f) with its vendored umsgpack:
/// `umsgpack.packb([1700000000.5, Identity.truncated_hash(b"/x"),
/// {"k": [1, "s", b"\x00"]}])`.
const REQUEST_KAT: &str = concat!(
    "93cb41d954fc40200000",
    "c410b3d1db318671a024a7e4b433389f8820",
    "81a16b9301a173c40100",
);

/// `Identity.truncated_hash(b"/x")`.
const PATH_HASH_OF_X: &str = "b3d1db318671a024a7e4b433389f8820";

/// `Identity.truncated_hash(req)` over the request vector: the id the
/// request carries when it travels as a resource.
const REQUEST_ID_OF_KAT: &str = "8a3f9fdb208b6ef1a27cb0672940b2fa";

/// Packed by the same checkout: `umsgpack.packb([rid, {"ok": True, "n": -7,
/// "f": 2.5, "b": b"\x01\x02"}])` with `rid = Identity.truncated_hash(req)`.
const RESPONSE_KAT: &str = concat!(
    "92c4108a3f9fdb208b6ef1a27cb0672940b2fa",
    "84a26f6bc3a16ef9a166cb4004000000000000a162c4020102",
);

/// The advertisement vectors `resource/tests_advert_kat.rs` pins, fed to the
/// smoke test as seeds.
const ADVERT_KAT: &str = concat!(
    "8ba174ce00011170a164cdffffa16e02a168c420000102030405060708090a0b0c0d0e0f",
    "101112131415161718191a1b1c1d1e1fa172c40410111213a16fc420202122232425262728",
    "292a2b2c2d2e2f303132333435363738393a3b3c3d3e3fa16900a16c01a171c41040414243",
    "4445464748494a4b4c4d4e4fa1662ba16dc4085051525354555657",
);

fn hex(text: &str) -> Vec<u8> {
    hex::decode(text).expect("pinned Python hex")
}

fn hash16(text: &str) -> [u8; ADDRESS_HASH_SIZE] {
    hex(text).try_into().expect("16 bytes")
}

fn kat_request_data() -> Value {
    Value::Map(vec![(
        Value::from("k"),
        Value::Array(vec![Value::from(1), Value::from("s"), Value::Binary(vec![0])]),
    )])
}

fn kat_response_body() -> Value {
    Value::Map(vec![
        (Value::from("ok"), Value::from(true)),
        (Value::from("n"), Value::from(-7)),
        (Value::from("f"), Value::F64(2.5)),
        (Value::from("b"), Value::Binary(vec![1, 2])),
    ])
}

/// `levels` fixarray-of-one markers around `leaf`.
fn nested_arrays(levels: usize, leaf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x91; levels];
    bytes.extend_from_slice(leaf);
    bytes
}

/// A request frame whose `data` is `leaf` wrapped in `levels` arrays.
fn request_around(levels: usize, leaf: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0x93, 0xcb];
    bytes.extend_from_slice(&1_700_000_000.5_f64.to_be_bytes());
    bytes.extend_from_slice(&[0xc4, 0x10]);
    bytes.extend_from_slice(&hash16(PATH_HASH_OF_X));
    bytes.extend_from_slice(&nested_arrays(levels, leaf));
    bytes
}

/// Each length prefix claims 2^31 bytes or entries on a ten-byte buffer.
fn oversize_prefix_inputs() -> [(&'static str, Vec<u8>, FrameDecodeError); 5] {
    fn padded(head: &[u8]) -> Vec<u8> {
        let mut bytes = head.to_vec();
        bytes.resize(10, 0);
        bytes
    }
    [
        ("bin32", padded(&[0xc6, 0x80, 0, 0, 0]), FrameDecodeError::LengthExceedsInput),
        ("str32", padded(&[0xdb, 0x80, 0, 0, 0]), FrameDecodeError::LengthExceedsInput),
        ("array32", padded(&[0xdd, 0x80, 0, 0, 0]), FrameDecodeError::Truncated),
        ("map32", padded(&[0xdf, 0x80, 0, 0, 0]), FrameDecodeError::Truncated),
        ("ext32", padded(&[0xc9, 0x80, 0, 0, 0, 0]), FrameDecodeError::LengthExceedsInput),
    ]
}

#[test]
fn a_request_frame_encodes_to_the_bytes_request_payload_packs() {
    let data = kat_request_data();
    let request = Link::request_payload("/page/index.mu", data.clone()).expect("packs");

    let frame = RequestFrame::decode(&request.packed).expect("the payload is a request frame");
    let rebuilt = RequestFrame::new(frame.requested_at(), PathHash::of("/page/index.mu"), data);

    assert_eq!(rebuilt.encode(), request.packed);
    assert_eq!(request.path_hash, frame.path_hash().to_bytes());
    assert_eq!(
        request.resource_request_id,
        RequestId::of_packed_request(&request.packed).to_bytes()
    );
}

#[test]
fn the_python_request_vector_decodes_and_re_encodes_byte_for_byte() {
    let bytes = hex(REQUEST_KAT);
    assert_eq!(bytes[1], 0xcb, "requested_at travels as float64");
    assert_eq!(PathHash::of("/x").to_bytes(), hash16(PATH_HASH_OF_X));

    let frame = RequestFrame::decode(&bytes).expect("decodes");

    assert_eq!(frame, RequestFrame::new(1_700_000_000.5, PathHash::of("/x"), kat_request_data()));
    assert_eq!(frame.encode(), bytes);
    assert_eq!(RequestId::of_packed_request(&bytes), RequestId::new(hash16(REQUEST_ID_OF_KAT)));
}

#[test]
fn the_python_response_vector_decodes_and_re_encodes_byte_for_byte() {
    let bytes = hex(RESPONSE_KAT);

    let frame = ResponseFrame::decode(&bytes).expect("decodes");

    assert_eq!(
        frame,
        ResponseFrame::new(RequestId::new(hash16(REQUEST_ID_OF_KAT)), kat_response_body())
    );
    assert_eq!(frame.encode(), bytes);
}

#[test]
fn a_response_frame_agrees_with_unpack_response_envelope() {
    let bytes = hex(RESPONSE_KAT);
    let frame = ResponseFrame::decode(&bytes).expect("decodes");
    let (request_id, body) = unpack_response_envelope(&bytes).expect("unpacks");
    assert_eq!(request_id, frame.request_id().to_bytes());
    assert_eq!(&body, frame.body());

    let mut trailing = bytes.clone();
    trailing.push(0x00);
    let mut three = vec![0x93];
    three.extend_from_slice(&bytes[1..]);
    three.push(0xc0);
    let mut short_id = vec![0x92, 0xc4, 0x0f];
    short_id.extend_from_slice(&[0x42; 15]);
    short_id.push(0xc0);
    // `rmpv::Value::as_slice` read a `str` id too, so the old decoder
    // accepted this one.
    let mut str_id = vec![0x92, 0xb0];
    str_id.extend_from_slice(&[b'a'; 16]);
    str_id.push(0xc0);
    for (label, input, expected) in [
        ("a trailing byte", trailing, FrameDecodeError::TrailingBytes),
        ("three elements", three, FrameDecodeError::WrongArity { expected: 2, found: 3 }),
        ("a 15-byte id", short_id, FrameDecodeError::HashNotBin16),
        ("a 16-byte str id", str_id, FrameDecodeError::HashNotBin16),
    ] {
        assert_eq!(ResponseFrame::decode(&input), Err(expected), "{label}");
        assert!(
            matches!(unpack_response_envelope(&input), Err(RnsError::PacketError)),
            "{label} collapses to PacketError"
        );
    }
}

#[test]
fn requested_at_is_refused_unless_it_is_a_float64() {
    let mut single = vec![0x93, 0xca];
    single.extend_from_slice(&1.5_f32.to_be_bytes());
    single.extend_from_slice(&[0xc4, 0x10]);
    single.extend_from_slice(&hash16(PATH_HASH_OF_X));
    single.push(0xc0);
    let mut integer = vec![0x93, 0x01, 0xc4, 0x10];
    integer.extend_from_slice(&hash16(PATH_HASH_OF_X));
    integer.push(0xc0);

    assert_eq!(RequestFrame::decode(&single), Err(FrameDecodeError::RequestedAtNotF64));
    assert_eq!(RequestFrame::decode(&integer), Err(FrameDecodeError::RequestedAtNotF64));
    assert!(RequestFrame::decode(&request_around(0, &[0xc0])).is_ok());
}

#[test]
fn a_request_path_hash_must_be_a_sixteen_byte_bin() {
    let mut short = vec![0x93, 0xcb];
    short.extend_from_slice(&0.0_f64.to_be_bytes());
    short.extend_from_slice(&[0xc4, 0x0f]);
    short.extend_from_slice(&[0x11; 15]);
    short.push(0xc0);
    let mut string = vec![0x93, 0xcb];
    string.extend_from_slice(&0.0_f64.to_be_bytes());
    string.push(0xb0);
    string.extend_from_slice(&[b'a'; 16]);
    string.push(0xc0);

    assert_eq!(RequestFrame::decode(&short), Err(FrameDecodeError::HashNotBin16));
    assert_eq!(RequestFrame::decode(&string), Err(FrameDecodeError::HashNotBin16));
}

#[test]
fn a_frame_must_be_an_array_of_the_right_arity() {
    assert_eq!(RequestFrame::decode(&[0xc0]), Err(FrameDecodeError::NotAnArray));
    assert_eq!(ResponseFrame::decode(&[0x80]), Err(FrameDecodeError::NotAnArray));
    assert_eq!(
        RequestFrame::decode(&hex(RESPONSE_KAT)),
        Err(FrameDecodeError::WrongArity { expected: 3, found: 2 })
    );
    assert_eq!(
        ResponseFrame::decode(&hex(REQUEST_KAT)),
        Err(FrameDecodeError::WrongArity { expected: 2, found: 3 })
    );
}

#[test]
fn containers_may_nest_128_deep_and_no_deeper() {
    assert_eq!(validate(&nested_arrays(MAX_DEPTH, &[0xc0])), Ok(()));
    assert_eq!(
        validate(&nested_arrays(MAX_DEPTH + 1, &[0xc0])),
        Err(FrameDecodeError::DepthExceeded)
    );
    let mut maps = vec![0x81, 0xc0];
    maps.extend_from_slice(&nested_arrays(MAX_DEPTH - 1, &[0xc0]));
    assert_eq!(validate(&maps), Ok(()));
    maps.insert(2, 0x91);
    assert_eq!(validate(&maps), Err(FrameDecodeError::DepthExceeded));

    let deepest = RequestFrame::decode(&request_around(MAX_DEPTH - 1, &[0xc0])).expect("depth 128");
    assert_eq!(deepest.encode(), request_around(MAX_DEPTH - 1, &[0xc0]));
    assert_eq!(
        RequestFrame::decode(&request_around(MAX_DEPTH, &[0xc0])),
        Err(FrameDecodeError::DepthExceeded)
    );
}

/// The allocator cannot be instrumented here (`unsafe_code` is forbidden, so
/// no counting global allocator); the proof that a refusal allocates nothing
/// is the walker's construction (it reads only the slice it is given), the
/// typed error it returns, and the time bound asserted below.
#[test]
fn oversize_length_prefixes_are_refused_before_anything_is_read() {
    for (label, input, expected) in oversize_prefix_inputs() {
        let started = Instant::now();
        assert_eq!(validate(&input), Err(expected), "{label}");
        assert_eq!(RequestFrame::decode(&input), Err(expected), "{label} as a request");
        assert_eq!(ResponseFrame::decode(&input), Err(expected), "{label} as a response");
        assert!(started.elapsed() < Duration::from_secs(1), "{label} refused without walking 2^31");
    }
}

#[test]
fn truncated_input_a_reserved_marker_and_trailing_bytes_are_refused() {
    let request = hex(REQUEST_KAT);
    for cut in 0..request.len() {
        let error = RequestFrame::decode(&request[..cut]).expect_err("a cut frame is refused");
        assert!(
            matches!(error, FrameDecodeError::Truncated | FrameDecodeError::LengthExceedsInput),
            "cut at {cut}: {error:?}"
        );
    }
    // Inside the timestamp the input ends mid-number; inside the path hash
    // the bin length prefix points past the end.
    assert_eq!(RequestFrame::decode(&request[..5]), Err(FrameDecodeError::Truncated));
    assert_eq!(RequestFrame::decode(&request[..20]), Err(FrameDecodeError::LengthExceedsInput));
    let mut trailing = request.clone();
    trailing.push(0xc0);
    assert_eq!(RequestFrame::decode(&trailing), Err(FrameDecodeError::TrailingBytes));
    assert_eq!(validate(&[0x91, 0xc1]), Err(FrameDecodeError::ReservedMarker));
    assert_eq!(validate(&[0xc7, 0x00]), Err(FrameDecodeError::Truncated));
    assert_eq!(validate(&[0xd4, 0x01, 0x02]), Ok(()));
}

#[test]
fn frame_decode_errors_collapse_to_packet_error() {
    assert!(matches!(RnsError::from(FrameDecodeError::DepthExceeded), RnsError::PacketError));
    assert!(matches!(RnsError::from(FrameDecodeError::HashNotBin16), RnsError::PacketError));
}

#[test]
fn a_packet_form_request_id_is_the_first_sixteen_bytes_of_the_packet_hash() {
    let hash = Hash::new_from_slice(b"a request packet");
    let id = RequestId::from_packet_hash(&hash);
    assert_eq!(id.as_bytes()[..], hash.as_slice()[..ADDRESS_HASH_SIZE]);
    assert_eq!(RequestId::from(id.to_bytes()), id);
}

#[test]
fn hashes_display_as_lowercase_hex() {
    assert_eq!(PathHash::of("/x").to_string(), PATH_HASH_OF_X);
    assert_eq!(RequestId::new(hash16(REQUEST_ID_OF_KAT)).to_string(), REQUEST_ID_OF_KAT);
    assert_eq!(PathHash::from(hash16(PATH_HASH_OF_X)).as_bytes(), &hash16(PATH_HASH_OF_X));
}

fn decode_everything(bytes: &[u8]) {
    match RequestFrame::decode(bytes) {
        Ok(frame) => {
            let encoded = frame.encode();
            let again = RequestFrame::decode(&encoded).expect("re-encoded request decodes");
            assert_eq!(again.encode(), encoded);
        }
        Err(FrameDecodeError::Malformed) => {
            panic!("the walk accepted an input the value reader refused")
        }
        Err(_) => {}
    }
    match ResponseFrame::decode(bytes) {
        Ok(frame) => {
            let encoded = frame.encode();
            let again = ResponseFrame::decode(&encoded).expect("re-encoded response decodes");
            assert_eq!(again.encode(), encoded);
        }
        Err(FrameDecodeError::Malformed) => {
            panic!("the walk accepted an input the value reader refused")
        }
        Err(_) => {}
    }
    if let Ok(advert) = ResourceAdvertisement::unpack(bytes) {
        let packed = advert.pack().expect("decoded advertisement packs");
        assert_eq!(
            ResourceAdvertisement::unpack(&packed).expect("re-packed advertisement unpacks"),
            advert
        );
    }
}

#[test]
fn fuzz_smoke_request_frames_and_advertisement_decoders_do_not_panic() {
    let mut corpus: Vec<Vec<u8>> = Vec::new();
    for kat in [REQUEST_KAT, RESPONSE_KAT, ADVERT_KAT] {
        let bytes = hex(kat);
        for cut in 0..=bytes.len() {
            corpus.push(bytes[..cut].to_vec());
        }
    }
    corpus.extend(oversize_prefix_inputs().into_iter().map(|(_, input, _)| input));
    corpus.push(nested_arrays(MAX_DEPTH + 1, &[0xc0]));
    corpus.push(request_around(MAX_DEPTH, &[0xc0]));
    for bytes in &corpus {
        decode_everything(bytes);
    }

    let mut seed = 0xA5A5_5A5A_1234_5678_u64;
    for _ in 0..6_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let len = ((seed >> 16) as usize) % 1024;
        let mut bytes = vec![0_u8; len];
        let mut stream = seed ^ 0x9E37_79B9_7F4A_7C15;
        for byte in &mut bytes {
            stream = stream.rotate_left(9).wrapping_mul(0xD134_2543_DE82_E285);
            *byte = (stream & 0xFF) as u8;
        }
        decode_everything(&bytes);
    }
}
