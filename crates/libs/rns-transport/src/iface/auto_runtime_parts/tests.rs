#[cfg(test)]
mod tests {
// Test code holds guards across awaits on purpose.
#![allow(clippy::await_holding_invalid_type)]

use crate::buffer::InputBuffer;
use crate::iface::auto::{AutoDiscoveryScope, MulticastAddressType};
include!("tests_sections/core_tests.rs");
include!("tests_sections/auto_multicast_discovery_bind_resolv.rs");
include!("tests_sections/auto_peer_data_transport_bridge_regi.rs");
include!("tests_sections/auto_peer_route_origination.rs");
include!("tests_sections/peer_route_table_expiry.rs");
include!("tests_sections/peer_route_table_probe.rs");
include!("tests_sections/peer_data_receive_loop_progress.rs");
include!("tests_sections/peer_data_receive_loop_progress_probe.rs");
}
