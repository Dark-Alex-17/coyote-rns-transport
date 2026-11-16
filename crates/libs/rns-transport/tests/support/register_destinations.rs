//! Registers a run of destinations under fresh identities on one transport; shared by the
//! integration tests that announce several destinations from one node.

use std::sync::Arc;

use rns_transport::destination::{DestinationName, SingleInputDestination};
use rns_transport::hash::AddressHash;
use rns_transport::identity::PrivateIdentity;
use rns_transport::rand_core::OsRng;
use rns_transport::transport::Transport;
use tokio::sync::Mutex;

/// Registers `count` destinations named `<prefix>0`, `<prefix>1`, ... under `app_name`, each
/// with a fresh identity, on `transport`; returns each with its address hash.
#[deny(clippy::await_holding_invalid_type)]
pub async fn register_destinations(
    transport: &Transport,
    app_name: &str,
    prefix: &str,
    count: usize,
) -> Vec<(Arc<Mutex<SingleInputDestination>>, AddressHash)> {
    let mut out = Vec::with_capacity(count);
    for idx in 0..count {
        let destination = transport
            .add_destination(
                PrivateIdentity::new_from_rand(OsRng),
                DestinationName::new(app_name, &format!("{prefix}{idx}")),
            )
            .await;
        let hash = destination.lock().await.desc.address_hash;
        out.push((destination, hash));
    }
    out
}
