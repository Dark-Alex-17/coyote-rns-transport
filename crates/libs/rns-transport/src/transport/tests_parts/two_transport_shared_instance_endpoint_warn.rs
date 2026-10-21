// Pins the one warning an attach can emit on its own: a TCP endpoint whose address is not
// loopback is accepted and warned about once, naming the address, since Python's
// `LocalClientInterface` connects to 127.0.0.1 only (`RNS/Interfaces/LocalInterface.py:109`).
// That a loopback attach never warns is pinned by
// `shared_instance_tcp_attach_logs_the_reconnect_wait_in_force_and_never_warns` in
// `two_transport_shared_instance_probe5.rs`, read through the same record store. Helpers
// come from the sibling `tests_parts` files included into this module.

use std::net::{IpAddr, Ipv4Addr};

/// An address nothing answers on: the attached `TcpClient` keeps redialing it in the
/// background, so the test's detach and stop run against a client that never connected.
const SHARED_INSTANCE_UNREACHABLE_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 255, 255, 1)), 1);

/// The attach warns once, before it spawns anything, with the endpoint in the text, and
/// then proceeds: the slot, the posture and the interface are those of any attach. The
/// client still redialing in the background holds up neither the detach nor the stop, and
/// neither adds a warning of its own.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_a_non_loopback_endpoint_warns_once_and_proceeds() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("a non-loopback endpoint is accepted");
    shared_instance_assert_attached(&transport, iface).await;

    let warned: Vec<String> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level == log::Level::Warn)
        .map(|(_, record)| record)
        .collect();
    assert_eq!(warned.len(), 1, "one warning per non-loopback attach: {warned:?}");
    let line = &warned[0];
    assert!(
        line.contains(&SHARED_INSTANCE_UNREACHABLE_ENDPOINT.to_string()),
        "names the endpoint: {line}"
    );
    assert!(line.contains("shared instance"), "is a shared-instance line: {line}");
    assert!(line.contains("not a loopback address"), "says why: {line}");
    assert!(line.contains("RNS/Interfaces/LocalInterface.py:109"), "cites Python: {line}");

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    let loud = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .count();
    assert_eq!(loud, 1, "the detach and the stop of a never-connected client add no warning");
}
