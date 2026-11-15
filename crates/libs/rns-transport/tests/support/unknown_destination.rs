//! Builds a destination no transport has a path to; shared by the integration tests that
//! open a link that can never come up.

use rns_transport::destination::{DestinationDesc, DestinationName};
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;

/// An `lxmf.delivery` destination of a fresh random identity, so no path to it exists.
pub fn unknown_destination() -> DestinationDesc {
    let remote = *PrivateIdentity::new_from_rand(OsRng).as_identity();
    DestinationDesc {
        identity: remote,
        address_hash: remote.address_hash,
        name: DestinationName::new("lxmf", "delivery"),
    }
}
