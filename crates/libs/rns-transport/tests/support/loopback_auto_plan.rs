//! An AutoInterface plan with one loopback discovery listener and one loopback data
//! listener on fresh ports, no adopted devices and no initial peering wait, so a datagram
//! from 127.0.0.1 reads as a remote peer at once; shared by the integration tests that
//! spawn a loopback AutoInterface runtime.

use std::time::Duration;

use rns_transport::iface::auto::{
    AutoDataListenerBinding, AutoDiscoveryListenerBinding, AutoDiscoveryScope, AutoInterfaceConfig,
    AutoInterfaceDeviceFilter, MulticastAddressType,
};
use rns_transport::iface::auto_runtime::AutoRuntimePlan;

/// Returns the plan and the unicast discovery port to send to.
pub fn loopback_auto_plan(group_id: &str) -> (AutoRuntimePlan, u16) {
    let data_reservation = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .expect("reserve data loopback UDP port");
    let data_port = data_reservation.local_addr().expect("read data port").port();
    let discovery_port = loop {
        let reservation = std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .expect("reserve discovery loopback UDP port");
        let port = reservation.local_addr().expect("read discovery port").port();
        if port != u16::MAX && port != data_port {
            break port;
        }
    };
    drop(data_reservation);
    let config = AutoInterfaceConfig {
        group_id: group_id.to_string(),
        discovery_scope: AutoDiscoveryScope::Global,
        multicast_address_type: MulticastAddressType::Permanent,
        discovery_port,
        data_port,
    };
    let filter = AutoInterfaceDeviceFilter { allowed: Vec::new(), ignored: Vec::new() };
    let mut plan = AutoRuntimePlan::from_candidates(config, filter, Vec::new());
    plan.startup_plan.initial_peering_wait = Duration::ZERO;
    plan.startup_plan.discovery_listeners.push(AutoDiscoveryListenerBinding {
        ifname: "lo".to_string(),
        link_local_address: "127.0.0.1".to_string(),
        unicast_bind_address: "127.0.0.1".to_string(),
        unicast_bind_port: discovery_port,
        multicast_group_address: "239.255.0.1".to_string(),
        multicast_bind_address: "239.255.0.1".to_string(),
        multicast_bind_port: discovery_port,
    });
    plan.startup_plan.data_listeners.push(AutoDataListenerBinding {
        ifname: "lo".to_string(),
        link_local_address: "127.0.0.1".to_string(),
        bind_address: "127.0.0.1".to_string(),
        bind_port: data_port,
    });
    (plan, discovery_port)
}
