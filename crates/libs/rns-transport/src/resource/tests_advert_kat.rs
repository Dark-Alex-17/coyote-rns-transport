// Kept out of `resource/tests.rs` because that file is on the module-size
// allowlist and should not grow further.

/// Packed by RNS 1.5.2 (ea98db4f) with its vendored umsgpack, keys in the
/// order `ResourceAdvertisement.pack` builds its dictionary
/// (`RNS/Resource.py:1337-1347`), flags as `RNS/Resource.py:1318` folds
/// them: `umsgpack.packb({"t": 70000, "d": 65535, "n": 2, "h":
/// bytes(range(32)), "r": bytes(range(0x10,0x14)), "o":
/// bytes(range(0x20,0x40)), "i": 0, "l": 1, "q": bytes(range(0x40,0x50)),
/// "f": 1<<5 | 0<<4 | 1<<3 | 0<<2 | 1<<1 | 1, "m": bytes(range(0x50,0x58))})`.
const ADVERT_WITH_REQUEST_ID: &str = concat!(
    "8ba174ce00011170a164cdffffa16e02a168c420000102030405060708090a0b0c0d0e0f",
    "101112131415161718191a1b1c1d1e1fa172c40410111213a16fc420202122232425262728",
    "292a2b2c2d2e2f303132333435363738393a3b3c3d3e3fa16900a16c01a171c41040414243",
    "4445464748494a4b4c4d4e4fa1662ba16dc4085051525354555657",
);

/// The same dictionary with `"q": None`.
const ADVERT_WITHOUT_REQUEST_ID: &str = concat!(
    "8ba174ce00011170a164cdffffa16e02a168c420000102030405060708090a0b0c0d0e0f",
    "101112131415161718191a1b1c1d1e1fa172c40410111213a16fc420202122232425262728",
    "292a2b2c2d2e2f303132333435363738393a3b3c3d3e3fa16900a16c01a171c0a1662ba16d",
    "c4085051525354555657",
);

/// The same dictionary as the second segment of a split response: `"i": 1,
/// "l": 2, "f": 0<<5 | 1<<4 | 0<<3 | 1<<2 | 0<<1 | 1`.
const ADVERT_SPLIT_RESPONSE: &str = concat!(
    "8ba174ce00011170a164cdffffa16e02a168c420000102030405060708090a0b0c0d0e0f",
    "101112131415161718191a1b1c1d1e1fa172c40410111213a16fc420202122232425262728",
    "292a2b2c2d2e2f303132333435363738393a3b3c3d3e3fa16901a16c02a171c41040414243",
    "4445464748494a4b4c4d4e4fa16615a16dc4085051525354555657",
);

const KEY_ORDER: [u8; 11] = *b"tdnhroilqfm";

fn range_bytes(range: std::ops::Range<u8>) -> Vec<u8> {
    range.collect()
}

fn expected_advertisement(request_id: Option<ByteBuf>) -> ResourceAdvertisement {
    ResourceAdvertisement {
        transfer_size: 70_000,
        data_size: 65_535,
        parts: 2,
        hash: Hash::new(range_bytes(0x00..0x20).try_into().expect("32 bytes")),
        random_hash: [0x10, 0x11, 0x12, 0x13],
        original_hash: Hash::new(range_bytes(0x20..0x40).try_into().expect("32 bytes")),
        segment_index: 0,
        total_segments: 1,
        request_id,
        flags: FLAG_METADATA | FLAG_REQUEST | FLAG_COMPRESSED | FLAG_ENCRYPTED,
        hashmap: range_bytes(0x50..0x58),
    }
}

/// Offset of the first `0xa1 <key>` pair in `packed`. None of the vectors'
/// values contain `0xa1`, so the first occurrence is the key itself.
fn key_offset(packed: &[u8], key: u8) -> usize {
    (0..packed.len() - 1)
        .find(|&i| packed[i] == 0xa1 && packed[i + 1] == key)
        .unwrap_or_else(|| panic!("key {}", key as char))
}

fn key_offsets(packed: &[u8]) -> Vec<usize> {
    KEY_ORDER.iter().map(|&key| key_offset(packed, key)).collect()
}

#[test]
fn the_python_advertisement_vector_unpacks_to_its_fields_and_packs_back_byte_for_byte() {
    let packed = hex::decode(ADVERT_WITH_REQUEST_ID).expect("pinned Python advertisement hex");
    let request_id = Some(ByteBuf::from(range_bytes(0x40..0x50)));

    let advertisement = ResourceAdvertisement::unpack(&packed).expect("unpacks");

    assert_eq!(advertisement, expected_advertisement(request_id));
    assert_eq!(advertisement.flags, 0x2b);
    assert_eq!(advertisement.pack().expect("packs"), packed);
}

#[test]
fn an_advertisement_without_a_request_id_packs_nil_for_q() {
    let packed = hex::decode(ADVERT_WITHOUT_REQUEST_ID).expect("pinned Python advertisement hex");

    let advertisement = ResourceAdvertisement::unpack(&packed).expect("unpacks");

    assert_eq!(advertisement, expected_advertisement(None));
    assert_eq!(advertisement.pack().expect("packs"), packed);
    let q = key_offset(&packed, b'q');
    assert_eq!(packed[q + 2], 0xc0, "`q` is followed by nil");
}

#[test]
fn a_split_response_segment_unpacks_its_response_and_split_bits_and_packs_back() {
    let packed = hex::decode(ADVERT_SPLIT_RESPONSE).expect("pinned Python advertisement hex");
    let request_id = Some(ByteBuf::from(range_bytes(0x40..0x50)));

    let advertisement = ResourceAdvertisement::unpack(&packed).expect("unpacks");

    assert_eq!(
        advertisement,
        ResourceAdvertisement {
            segment_index: 1,
            total_segments: 2,
            flags: FLAG_RESPONSE | FLAG_SPLIT | FLAG_ENCRYPTED,
            ..expected_advertisement(request_id)
        }
    );
    assert_eq!(advertisement.pack().expect("packs"), packed);
    assert!(advertisement.is_response());
    assert!(!advertisement.is_request());
    assert_ne!(advertisement.flags & FLAG_SPLIT, 0);
    let f = key_offset(&packed, b'f');
    let (x, p, u, s, c, e) = (0u8, 1u8, 0u8, 1u8, 0u8, 1u8);
    assert_eq!(packed[f + 2], x << 5 | p << 4 | u << 3 | s << 2 | c << 1 | e);
    assert_eq!(packed[f + 2], 0x15);
}

#[test]
fn an_advertisement_packs_its_keys_in_the_reference_order() {
    for vector in [ADVERT_WITH_REQUEST_ID, ADVERT_WITHOUT_REQUEST_ID, ADVERT_SPLIT_RESPONSE] {
        let packed = hex::decode(vector).expect("pinned Python advertisement hex");
        assert_eq!(packed[0], 0x8b, "a fixmap of eleven entries");
        let offsets = key_offsets(&packed);
        assert_eq!(offsets[0], 1, "`t` is the first key");
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]), "{offsets:?}");
    }
}

#[test]
fn advertisement_flags_fold_as_the_reference_bit_layout() {
    let packed = hex::decode(ADVERT_WITH_REQUEST_ID).expect("pinned Python advertisement hex");
    let advertisement = ResourceAdvertisement::unpack(&packed).expect("unpacks");
    let f = key_offset(&packed, b'f');
    // `x p u s c e` as `RNS/Resource.py:1318` folds them.
    let (x, p, u, s, c, e) = (1u8, 0u8, 1u8, 0u8, 1u8, 1u8);
    assert_eq!(packed[f + 2], x << 5 | p << 4 | u << 3 | s << 2 | c << 1 | e);

    assert_eq!(FLAG_METADATA, 1 << 5);
    assert_eq!(FLAG_RESPONSE, 1 << 4);
    assert_eq!(FLAG_REQUEST, 1 << 3);
    assert_eq!(FLAG_SPLIT, 1 << 2);
    assert_eq!(FLAG_COMPRESSED, 1 << 1);
    assert_eq!(FLAG_ENCRYPTED, 1);
    assert!(advertisement.encrypted());
    assert!(advertisement.compressed());
    assert!(advertisement.is_request());
    assert!(!advertisement.is_response());
    assert_eq!(advertisement.flags & FLAG_SPLIT, 0);
    assert_eq!(advertisement.flags & FLAG_METADATA, FLAG_METADATA);
}
