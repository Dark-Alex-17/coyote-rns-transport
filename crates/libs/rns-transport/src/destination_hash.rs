use std::io::{Error, ErrorKind};

use sha2::Digest;

use crate::destination::NAME_HASH_LENGTH;
use crate::hash::{AddressHash, Hash};

pub fn parse_destination_hash(input: &str) -> Option<[u8; 16]> {
    let bytes = hex::decode(input.trim()).ok()?;
    let mut out = [0u8; 16];
    match bytes.len() {
        16 => {
            out.copy_from_slice(&bytes);
            Some(out)
        }
        32 => {
            out.copy_from_slice(&bytes[..16]);
            Some(out)
        }
        _ => None,
    }
}

pub fn parse_destination_hash_required(input: &str) -> Result<[u8; 16], Error> {
    parse_destination_hash(input).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("invalid destination hash '{input}' (expected 16-byte or 32-byte hex)"),
        )
    })
}

/// Destination address for a name hash and an identity hash, as RNS/Destination.py:116-130
/// computes it: SHA-256 over `name_hash ++ identity_hash`, truncated to the address length.
pub fn destination_address(
    name_hash: &[u8; NAME_HASH_LENGTH],
    identity: &AddressHash,
) -> AddressHash {
    AddressHash::new_from_hash(&Hash::new(
        Hash::generator()
            .chain_update(name_hash)
            .chain_update(identity.as_slice())
            .finalize()
            .into(),
    ))
}

#[cfg(test)]
mod tests {
    use rand_core::OsRng;

    use super::destination_address;
    use crate::destination::{DestinationName, SingleInputDestination};
    use crate::identity::PrivateIdentity;

    #[test]
    fn destination_address_matches_single_input_destination() {
        for (app, aspect) in [("lxmf", "delivery"), ("app", "aspect"), ("rnx", "probe.v1")] {
            let identity = PrivateIdentity::new_from_rand(OsRng);
            let name = DestinationName::new(app, aspect);
            let name_hash = name.as_name_hash_slice().try_into().expect("name hash length");
            let expected = SingleInputDestination::new(identity.clone(), name).desc.address_hash;

            assert_eq!(
                destination_address(&name_hash, &identity.as_identity().address_hash),
                expected,
                "{app}.{aspect}"
            );
        }
    }
}
