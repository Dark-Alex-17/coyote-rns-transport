//! Builds a non-transport node with broadcast on; shared by the integration tests that
//! open a link with no path to its destination.

use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};

/// A non-transport `Transport` named `name` with a fresh identity and broadcast on, so
/// `Transport::link` to an unknown destination hands back a pending link.
pub fn broadcasting(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), true))
}
