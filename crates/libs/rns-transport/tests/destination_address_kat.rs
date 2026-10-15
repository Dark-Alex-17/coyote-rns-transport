//! Known-answer vectors for `destination_address`, computed independently with
//! Python `hashlib` following RNS/Destination.py:116-130 (`Destination.hash`):
//! `sha256(sha256(expanded_name)[:10] ++ identity_hash)[:16]`.
//!
//! Pinning the vectors here keeps the helper honest against the reference
//! algorithm rather than only against `SingleInputDestination`, which shares
//! its implementation.

use rns_transport::destination::DestinationName;
use rns_transport::destination_hash::destination_address;
use rns_transport::hash::AddressHash;

const VECTORS: [(&str, &str, &str, &str, &str); 3] = [
    (
        "lxmf",
        "delivery",
        "6ec60bc318e2c0f0d908",
        "000102030405060708090a0b0c0d0e0f",
        "a953c22cba1cff60cbaf4d3c75ecf35a",
    ),
    (
        "rnstransport",
        "probe",
        "fd68805f2ea383c8d6f6",
        "ffeeddccbbaa99887766554433221100",
        "87250087459c09a73f96ebe6feec1ba2",
    ),
    (
        "example_utilities",
        "linkexample",
        "2cbf2141101e5af83fee",
        "deadbeefcafebabe0123456789abcdef",
        "c91c12f9e845d5668891add443ddf0e2",
    ),
];

#[test]
fn destination_address_matches_python_reference_vectors() {
    for (app, aspect, expected_name_hash, identity_hex, expected_address) in VECTORS {
        let name = DestinationName::new(app, aspect);
        assert_eq!(
            hex::encode(name.as_name_hash_slice()),
            expected_name_hash,
            "name hash for {app}.{aspect}"
        );
        let name_hash: [u8; 10] = name.as_name_hash_slice().try_into().expect("name hash length");
        let identity = AddressHash::new_from_hex_string(identity_hex).expect("identity hex");

        let address = destination_address(&name_hash, &identity);
        assert_eq!(address.to_hex_string(), expected_address, "address for {app}.{aspect}");
        assert_eq!(address.short(), &expected_address[..8], "short form for {app}.{aspect}");
    }
}

#[test]
fn destination_address_round_trips_through_the_hex_parser() {
    let (app, aspect, _, identity_hex, expected_address) = VECTORS[0];
    let name = DestinationName::new(app, aspect);
    let name_hash: [u8; 10] = name.as_name_hash_slice().try_into().expect("name hash length");
    let identity = AddressHash::new_from_hex_string(identity_hex).expect("identity hex");

    let reparsed = AddressHash::new_from_hex_string(expected_address).expect("address hex");
    assert_eq!(destination_address(&name_hash, &identity), reparsed);
}
