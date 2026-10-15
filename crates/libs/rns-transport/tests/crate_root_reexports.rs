//! A downstream crate names the `rmpv` and `rand_core` versions the public API uses
//! through the crate root instead of pinning its own copies.

use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::destination_hash::destination_address;
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;

#[test]
fn rand_core_reexport_drives_the_public_constructors() {
    let identity = PrivateIdentity::new_from_rand(rns_transport::rand_core::OsRng);
    let hash = AddressHash::new_from_rand(rns_transport::rand_core::OsRng);
    assert_eq!(hash.short().len(), 8);
    assert!(hash.short().chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert_eq!(
        identity.as_identity().address_hash.short(),
        identity.as_identity().address_hash.to_hex_string()[..8]
    );
}

#[test]
fn rmpv_reexport_round_trips_a_value() {
    let value = rns_transport::rmpv::Value::Array(vec![
        rns_transport::rmpv::Value::from(7u8),
        rns_transport::rmpv::Value::from("seven"),
    ]);
    let mut buf = Vec::new();
    rns_transport::rmpv::encode::write_value(&mut buf, &value).expect("encode");
    let decoded = rns_transport::rmpv::decode::read_value(&mut buf.as_slice()).expect("decode");
    assert_eq!(decoded, value);
}

#[test]
fn destination_address_is_reachable_from_outside_the_crate() {
    let identity = PrivateIdentity::new_from_rand(rns_transport::rand_core::OsRng);
    let name = DestinationName::new("probe", "external");
    let name_hash: [u8; 10] = name.as_name_hash_slice().try_into().expect("name hash length");
    let expected = SingleInputDestination::new(identity.clone(), name).desc.address_hash;
    assert_eq!(destination_address(&name_hash, &identity.as_identity().address_hash), expected);

    let other = PrivateIdentity::new_from_rand(rns_transport::rand_core::OsRng);
    assert_ne!(destination_address(&name_hash, &other.as_identity().address_hash), expected);
}
