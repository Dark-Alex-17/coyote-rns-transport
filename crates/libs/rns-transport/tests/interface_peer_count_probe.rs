//! `Transport::interface_peer_count` beyond the `None` cases and the hand-held counter
//! that `interface_peer_count_api.rs` covers, driven through the public surface only: a
//! public `AutoDiscoveryState` whose counter `InterfaceManager::set_peer_counter` installs
//! on a host drives the host's count through the transport `Some(0)` -> `Some(1)` ->
//! `Some(2)` -> `Some(0)` as peers authenticate, refresh and expire (what Python reports
//! as `len(interface.peers)`, `RNS/Reticulum.py:1501-1503`); a `Clone` of the state
//! shares the counter; two hosts keep separate counters; a later install replaces the
//! earlier one on the host and its children; `stop_interface` on a child leaves the
//! host's count and on the host reads `None`; and an install aimed at a child's own
//! address touches only that child.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rns_transport::iface::auto::{
    peering_token, AutoDiscoveryEvent, AutoDiscoveryState, AutoPeerEvent,
};
use rns_transport::iface::IfaceRole;
use tokio::time::timeout;

#[path = "support/client_transport.rs"]
mod client_transport;
use client_transport::client_transport;

const WAIT: Duration = Duration::from_secs(5);
const GROUP_ID: &[u8] = b"peer-count-probe-group";
const PEERING_TIMEOUT: Duration = Duration::from_millis(200);
const REVERSE_PEERING_INTERVAL: Duration = Duration::from_millis(5_200);

/// A discovery state with no adopted devices and the short peering timeout, so every
/// loopback source reads as a remote peer and expiry is one `now` step away.
fn discovery_state() -> AutoDiscoveryState {
    AutoDiscoveryState::new(Vec::new(), PEERING_TIMEOUT, REVERSE_PEERING_INTERVAL)
}

/// Feeds one authenticated discovery datagram from `address` on `ifname` at `now`.
fn admit(
    state: &mut AutoDiscoveryState,
    address: &str,
    ifname: &str,
    now: Duration,
) -> AutoPeerEvent {
    let token = peering_token(GROUP_ID, address);
    match state
        .observe_authenticated_discovery_packet(&token, GROUP_ID, address, ifname, now)
        .expect("a token minted for its own source authenticates")
    {
        AutoDiscoveryEvent::Peer(event) => event,
        other => panic!("expected a peer event, got {other:?}"),
    }
}

/// Through the public surface: the host channel's count follows the discovery state's
/// peer table as peers are added, refreshed and expired, and a virtual child registered
/// on the host answers the same at every step.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn host_count_follows_a_public_discovery_state_through_add_refresh_and_expiry() {
    let transport = client_transport("peer-count-probe-state");
    let manager = transport.iface_manager();
    let host = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast).address;
    let child = manager
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register a child on the host");

    let mut state = discovery_state();
    assert_eq!(state.peer_count(), 0);
    assert_eq!(transport.interface_peer_count(&host).await, None, "nothing installed yet");
    assert!(manager.lock().await.set_peer_counter(&host, state.peer_counter()));
    assert_eq!(transport.interface_peer_count(&host).await, Some(0), "alone");
    assert_eq!(transport.interface_peer_count(&child).await, Some(0));

    let t0 = Duration::from_secs(10);
    assert_eq!(admit(&mut state, "fe80::1", "en0", t0), AutoPeerEvent::Added);
    assert_eq!(transport.interface_peer_count(&host).await, Some(1), "one discovery datagram");
    assert_eq!(transport.interface_peer_count(&child).await, Some(1));

    assert_eq!(admit(&mut state, "fe80::2", "en0", t0), AutoPeerEvent::Added);
    assert_eq!(transport.interface_peer_count(&host).await, Some(2));

    assert_eq!(
        admit(&mut state, "fe80::1", "en0", t0 + Duration::from_millis(100)),
        AutoPeerEvent::Refreshed
    );
    assert_eq!(transport.interface_peer_count(&host).await, Some(2), "a refresh adds nothing");

    // `fe80::2` was last heard at t0; `fe80::1` at t0 + 100 ms. One step past the
    // timeout for the older peer expires it alone.
    let expired = state.expire_stale_peers(t0 + PEERING_TIMEOUT + Duration::from_millis(50));
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].address, "fe80::2");
    assert_eq!(transport.interface_peer_count(&host).await, Some(1));

    let expired = state.expire_stale_peers(t0 + Duration::from_secs(1));
    assert_eq!(expired.len(), 1);
    assert_eq!(transport.interface_peer_count(&host).await, Some(0), "after the peer times out");
    assert_eq!(transport.interface_peer_count(&child).await, Some(0));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A `Clone` of the discovery state shares the counter, so a peer admitted on either
/// copy is read through the host, and each copy publishes its own length when it last
/// wrote.
#[test]
fn a_clone_of_the_discovery_state_shares_the_counter() {
    let mut original = discovery_state();
    let mut clone = original.clone();
    assert!(Arc::ptr_eq(&original.peer_counter(), &clone.peer_counter()));

    let counter = original.peer_counter();
    admit(&mut clone, "fe80::9", "en0", Duration::from_secs(1));
    assert_eq!(clone.peer_count(), 1);
    assert_eq!(original.peer_count(), 0, "the table itself is not shared");
    assert_eq!(counter.load(Ordering::Acquire), 1, "the clone wrote the shared counter");

    admit(&mut original, "fe80::8", "en0", Duration::from_secs(1));
    admit(&mut original, "fe80::7", "en0", Duration::from_secs(1));
    assert_eq!(counter.load(Ordering::Acquire), 2, "the last writer's length wins");
}

/// Two hosts on one transport keep separate counters, and installing on one leaves the
/// other reading `None`.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn two_hosts_keep_separate_counters() {
    let transport = client_transport("peer-count-probe-two-hosts");
    let manager = transport.iface_manager();
    let (lan, wifi) = {
        let mut manager = manager.lock().await;
        (
            manager.new_channel_with_role(8, IfaceRole::Multicast).address,
            manager.new_channel_with_role(8, IfaceRole::Multicast).address,
        )
    };
    let lan_peers = Arc::new(AtomicU64::new(3));
    assert!(manager.lock().await.set_peer_counter(&lan, Arc::clone(&lan_peers)));
    assert_eq!(transport.interface_peer_count(&lan).await, Some(3));
    assert_eq!(transport.interface_peer_count(&wifi).await, None, "the other host has none");

    let wifi_peers = Arc::new(AtomicU64::new(1));
    assert!(manager.lock().await.set_peer_counter(&wifi, Arc::clone(&wifi_peers)));
    assert_eq!(transport.interface_peer_count(&lan).await, Some(3));
    assert_eq!(transport.interface_peer_count(&wifi).await, Some(1));

    lan_peers.store(0, Ordering::Release);
    assert_eq!(transport.interface_peer_count(&lan).await, Some(0));
    assert_eq!(transport.interface_peer_count(&wifi).await, Some(1), "unaffected");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// A second install on the same host replaces the counter for the host and every child
/// (idempotent in shape: the call keeps returning `true`), and the replaced counter stops
/// being read.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn a_later_install_replaces_the_counter_on_host_and_children() {
    let transport = client_transport("peer-count-probe-reinstall");
    let manager = transport.iface_manager();
    let host = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast).address;
    let first = Arc::new(AtomicU64::new(1));
    assert!(manager.lock().await.set_peer_counter(&host, Arc::clone(&first)));
    let child = manager
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register a child after the first install");
    assert_eq!(transport.interface_peer_count(&child).await, Some(1));

    let second = Arc::new(AtomicU64::new(9));
    assert!(manager.lock().await.set_peer_counter(&host, Arc::clone(&second)));
    assert_eq!(transport.interface_peer_count(&host).await, Some(9));
    assert_eq!(transport.interface_peer_count(&child).await, Some(9), "the child follows");

    first.store(4, Ordering::Release);
    assert_eq!(transport.interface_peer_count(&host).await, Some(9), "the old counter is gone");
    second.store(2, Ordering::Release);
    assert_eq!(transport.interface_peer_count(&host).await, Some(2));
    assert_eq!(transport.interface_peer_count(&child).await, Some(2));

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// `stop_interface` on a virtual child removes that child's read and leaves the host's
/// count; `stop_interface` on the host makes the host read `None` like any pruned record.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn stopping_a_child_keeps_the_host_count_and_stopping_the_host_reads_none() {
    let transport = client_transport("peer-count-probe-stop");
    let manager = transport.iface_manager();
    let host = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast).address;
    let counter = Arc::new(AtomicU64::new(2));
    assert!(manager.lock().await.set_peer_counter(&host, Arc::clone(&counter)));
    let child = manager
        .lock()
        .await
        .register_virtual_iface(host, IfaceRole::VirtualUnicast)
        .expect("register a child");
    assert_eq!(transport.interface_peer_count(&child).await, Some(2));

    assert!(transport.stop_interface(child).await, "the child is a known interface");
    assert_eq!(transport.interface_peer_count(&child).await, None, "pruned child");
    assert_eq!(transport.interface_peer_count(&host).await, Some(2), "host unaffected");

    assert!(transport.stop_interface(host).await, "the host is a known interface");
    assert_eq!(transport.interface_peer_count(&host).await, None, "pruned host");
    assert_eq!(counter.load(Ordering::Acquire), 2, "the counter itself is untouched");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}

/// `set_peer_counter` installs for the record that carries the address it is given and
/// that record's children. Aimed at a virtual child, it returns `true` (a record carries
/// the address), installs on that child alone and leaves the host's counter in place.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn an_install_aimed_at_a_child_address_touches_only_that_child() {
    let transport = client_transport("peer-count-probe-child-install");
    let manager = transport.iface_manager();
    let host = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast).address;
    let host_counter = Arc::new(AtomicU64::new(1));
    assert!(manager.lock().await.set_peer_counter(&host, Arc::clone(&host_counter)));
    let (first_child, second_child) = {
        let mut manager = manager.lock().await;
        (
            manager
                .register_virtual_iface(host, IfaceRole::VirtualUnicast)
                .expect("register the first child"),
            manager
                .register_virtual_iface(host, IfaceRole::VirtualUnicast)
                .expect("register the second child"),
        )
    };

    let child_counter = Arc::new(AtomicU64::new(7));
    assert!(manager.lock().await.set_peer_counter(&first_child, Arc::clone(&child_counter)));
    assert_eq!(transport.interface_peer_count(&first_child).await, Some(7));
    assert_eq!(transport.interface_peer_count(&host).await, Some(1), "host keeps its own");
    assert_eq!(transport.interface_peer_count(&second_child).await, Some(1), "sibling too");

    timeout(WAIT, transport.stop()).await.expect("transport stop completes");
}
