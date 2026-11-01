//! A non-broadcasting transport under a fresh identity; shared by the integration tests
//! that attach a stream client to one.

use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::{Transport, TransportConfig};

pub fn client_transport(name: &str) -> Transport {
    Transport::new(TransportConfig::new(name, &PrivateIdentity::new_from_rand(OsRng), false))
}
