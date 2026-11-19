//! The `"peers"` key of `AutoRuntimeStatusHandle::to_json()`, driven from
//! outside the crate the way reticulumd and an embedder use it: present and `0` on a
//! fresh handle that no spawn has touched; equal to `peer_count()` and to
//! `Transport::interface_peer_count` on the bridge host once a spawned loopback runtime
//! admits a peer; the same through a clone; and still the last value after the runtime
//! stops, the retention the rustdocs promise (nothing publishes 0 on stop).

#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use std::time::Instant;

use rns_transport::iface::auto_runtime::AutoRuntimeStatusHandle;

#[cfg(unix)]
use rns_transport::iface::auto::peering_token;
#[cfg(unix)]
use rns_transport::iface::auto_runtime::AutoInterfaceTransportRuntime;
#[cfg(unix)]
use rns_transport::iface::IfaceRole;

#[cfg(unix)]
#[path = "support/client_transport.rs"]
mod client_transport;
#[path = "support/loopback_auto_plan.rs"]
mod loopback_auto_plan;
use loopback_auto_plan::loopback_auto_plan;

const GROUP_ID: &str = "peers-json-probe";

fn peers_key(handle: &AutoRuntimeStatusHandle) -> Option<u64> {
    let json = handle.to_json();
    assert!(
        json.get("peers").is_some(),
        "the peers key is always present in the handle JSON: {json}"
    );
    json["peers"].as_u64()
}

/// A handle no spawn has touched carries `"peers": 0` as an unsigned integer, equal to
/// `peer_count()`, on the handle and on a clone.
#[test]
fn a_fresh_handle_carries_peers_zero_in_its_json() {
    let (plan, _) = loopback_auto_plan(GROUP_ID);
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    assert_eq!(peers_key(&handle), Some(0));
    assert_eq!(peers_key(&handle), Some(handle.peer_count()));
    assert!(handle.to_json()["peers"].is_u64(), "peers is a JSON unsigned integer");
    assert_eq!(peers_key(&handle.clone()), Some(0));
}

#[cfg(unix)]
async fn wait_for_peers_key(handle: &AutoRuntimeStatusHandle, expected: u64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if peers_key(handle) == Some(expected) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for to_json()[\"peers\"] == {expected}: last {:?}",
            handle.to_json()["peers"]
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(unix)]
async fn send_peering_token(discovery_port: u16) {
    let socket = tokio::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind loopback peer socket");
    let token = peering_token(GROUP_ID.as_bytes(), "127.0.0.1");
    socket
        .send_to(&token, (std::net::Ipv4Addr::LOCALHOST, discovery_port))
        .await
        .expect("send peering token");
}

/// Spawned on a `Transport`'s manager with a transport bridge, the three public reads
/// agree at every step: `to_json()["peers"]`, `peer_count()` and
/// `Transport::interface_peer_count(&host)` go from `0` to `1` together; a clone of the
/// handle reads the same JSON; and after `runtime.stop()` the JSON still says `1` (the
/// last value, as documented) while the transport read keeps `Some(1)` until the host
/// record is pruned, after which it is `None` and the handle still says `1`.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn peers_json_tracks_the_transport_read_and_holds_its_last_value_after_stop() {
    let (plan, discovery_port) = loopback_auto_plan(GROUP_ID);
    let transport = client_transport::client_transport("peers-json-probe");
    let manager = transport.iface_manager();
    let channel = manager.lock().await.new_channel_with_role(8, IfaceRole::Multicast);
    let host = channel.address;
    let bridge =
        AutoInterfaceTransportRuntime::from_channel(channel, std::sync::Arc::clone(&manager));
    assert_eq!(transport.interface_peer_count(&host).await, None, "nothing installed yet");

    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    let observer = handle.clone();
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(
            Some(bridge),
            Some(handle.clone()),
        )
        .await
        .expect("start loopback AutoInterface runtime with a transport bridge");

    assert_eq!(peers_key(&handle), Some(0));
    assert_eq!(transport.interface_peer_count(&host).await, Some(0), "installed by the spawn");

    send_peering_token(discovery_port).await;
    wait_for_peers_key(&handle, 1).await;

    assert_eq!(handle.peer_count(), 1);
    assert_eq!(peers_key(&observer), Some(1), "a clone reads the same JSON");
    assert_eq!(
        transport.interface_peer_count(&host).await,
        Some(1),
        "the transport reads the same counter the JSON carries"
    );

    runtime.stop().await;
    assert_eq!(
        peers_key(&handle),
        Some(1),
        "stop publishes nothing: the JSON holds its last value"
    );
    assert_eq!(handle.peer_count(), 1);
    assert_eq!(
        transport.interface_peer_count(&host).await,
        Some(1),
        "the host record keeps the counter until it is pruned"
    );

    assert!(transport.stop_interface(host).await, "the host is a known interface");
    assert_eq!(transport.interface_peer_count(&host).await, None, "pruned host reads None");
    assert_eq!(peers_key(&handle), Some(1), "the handle outlives the record with the last value");

    tokio::time::timeout(Duration::from_secs(5), transport.stop())
        .await
        .expect("transport stop completes");
}

/// Spawned without a transport bridge, the JSON key still follows the admitted peer:
/// the handle is the whole in-process surface for an embedder that has no manager.
#[cfg(unix)]
#[tokio::test]
async fn peers_json_follows_an_admitted_peer_without_a_transport_bridge() {
    let (plan, discovery_port) = loopback_auto_plan(GROUP_ID);
    let handle = AutoRuntimeStatusHandle::from_startup_plan(&plan.startup_plan);
    let runtime = plan
        .spawn_discovery_runtime_with_native_scope_ids_and_transport(None, Some(handle.clone()))
        .await
        .expect("start loopback AutoInterface runtime");
    assert_eq!(peers_key(&handle), Some(0));

    send_peering_token(discovery_port).await;
    wait_for_peers_key(&handle, 1).await;
    assert_eq!(peers_key(&handle), Some(handle.peer_count()));

    runtime.stop().await;
    assert_eq!(peers_key(&handle), Some(1), "last value retained after stop");
}
