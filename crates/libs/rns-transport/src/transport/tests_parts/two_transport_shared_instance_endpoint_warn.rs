// Pins the loopback rule of a TCP attach: an endpoint whose address is not loopback is
// refused with `SharedInstanceEndpointNotLoopback` before any lock is taken, with one
// warning naming the address, and is attached only under `allow_remote_endpoint(true)`,
// again with one warning per accepted attach, since Python's `LocalClientInterface`
// connects to 127.0.0.1 only (`RNS/Interfaces/LocalInterface.py:109`); an attach refused by
// the slot warns nothing. That a loopback attach never warns is pinned by
// `shared_instance_tcp_attach_logs_the_reconnect_wait_in_force_and_never_warns` in
// `two_transport_shared_instance_probe5.rs`, read through the same record store. Helpers
// come from the sibling `tests_parts` files included into this module.

use std::net::{IpAddr, Ipv4Addr};

/// An address nothing answers on: the attached `TcpClient` keeps redialing it in the
/// background, so the test's detach and stop run against a client that never connected.
const SHARED_INSTANCE_UNREACHABLE_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 255, 255, 1)), 1);

/// The IPv4 loopback on a port nothing answers on.
const SHARED_INSTANCE_V4_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1);

/// The warnings recorded for `transport_name`.
fn shared_instance_warnings_for(transport_name: &str) -> Vec<String> {
    shared_instance_records_for(transport_name)
        .into_iter()
        .filter(|(level, _)| *level == log::Level::Warn)
        .map(|(_, record)| record)
        .collect()
}

/// With default options the attach is refused: the error names the rule, the slot stays
/// empty, no interface is registered, and the one warning names the endpoint, the refusal
/// and the Python line. The slot is untouched, so a loopback attach that follows succeeds.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_a_non_loopback_endpoint_is_refused_by_default() {
    install_test_logger();
    let name = "shared-instance-endpoint-refused";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let interfaces_before = transport.interface_hashes().await;
    let refused = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await;
    assert!(
        matches!(refused, Err(RnsError::SharedInstanceEndpointNotLoopback)),
        "a non-loopback endpoint is refused without the opt-in: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), None, "a refused attach takes no slot");
    assert!(!transport.connected_to_shared_instance().await, "the posture stays clear");
    assert_eq!(
        transport.interface_hashes().await,
        interfaces_before,
        "a refused attach registers no interface"
    );

    let warned = shared_instance_warnings_for(name);
    assert_eq!(warned.len(), 1, "one warning per refused attach: {warned:?}");
    let line = &warned[0];
    assert!(
        line.contains(&SHARED_INSTANCE_UNREACHABLE_ENDPOINT.to_string()),
        "names the endpoint: {line}"
    );
    assert!(line.contains("not a loopback address"), "says why: {line}");
    assert!(line.contains("attach refused"), "says what happened: {line}");
    assert!(line.contains("RNS/Interfaces/LocalInterface.py:109"), "cites Python: {line}");

    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_V4_LOOPBACK_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("a loopback attach after a refusal is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    assert_eq!(shared_instance_warnings_for(name).len(), 1, "the loopback attach adds none");

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}

/// Under `allow_remote_endpoint(true)` the attach warns once, with the endpoint and the
/// option in the text, and proceeds: the slot, the posture and the interface are those of
/// any attach. The client still redialing in the background holds up neither the detach
/// nor the stop, and neither adds a warning on the links target (the client's own connect
/// warning is on its own target, outside the oracle).
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_a_non_loopback_endpoint_under_the_opt_in_warns_once() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default().allow_remote_endpoint(true),
        )
        .await
        .expect("a non-loopback endpoint is accepted under the opt-in");
    shared_instance_assert_attached(&transport, iface).await;

    let warned = shared_instance_warnings_for(name);
    assert_eq!(warned.len(), 1, "one warning per accepted remote attach: {warned:?}");
    let line = &warned[0];
    assert!(
        line.contains(&SHARED_INSTANCE_UNREACHABLE_ENDPOINT.to_string()),
        "names the endpoint: {line}"
    );
    assert!(line.contains("shared instance"), "is a shared-instance line: {line}");
    assert!(line.contains("not a loopback address"), "says why: {line}");
    assert!(line.contains("allow_remote_endpoint"), "names the opt-in: {line}");
    assert!(line.contains("RNS/Interfaces/LocalInterface.py:109"), "cites Python: {line}");

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    let loud = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .count();
    assert_eq!(
        loud, 1,
        "the detach and the stop of a never-connected client add no warning on the links target"
    );
}

/// A second attach while the slot is held is refused before anything is spawned, and a
/// refused attach warns nothing even for a non-loopback endpoint under the opt-in: the
/// accepted-remote warning is emitted only once the slot has taken the interface.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_refused_attach_to_a_non_loopback_endpoint_does_not_warn() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn-refused";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default().allow_remote_endpoint(true),
        )
        .await
        .expect("a non-loopback endpoint is accepted under the opt-in");
    shared_instance_assert_attached(&transport, iface).await;
    assert_eq!(shared_instance_warnings_for(name).len(), 1, "the accepted attach warns once");

    let refused = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default().allow_remote_endpoint(true),
        )
        .await;
    assert!(matches!(refused, Err(RnsError::InvalidArgument)), "the slot is held: {refused:?}");
    assert_eq!(transport.shared_instance(), Some(iface), "the first attach keeps the slot");
    let loud = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .count();
    assert_eq!(loud, 1, "the refused attach adds no warning");

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}
