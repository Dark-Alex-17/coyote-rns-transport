// Pins the boundary of the loopback rule, read through the same record store as
// `two_transport_shared_instance_endpoint_warn.rs`: the rule keys on "loopback", not on
// `127.0.0.1`, so the IPv6 loopback `[::1]` attaches as quietly as the IPv4 one, an
// IPv4-mapped IPv6 address that is not loopback is refused like its IPv4 form, and the
// accepted-remote warning is per attach: under the opt-in a detach followed by a second
// attach to the same endpoint warns again, since each attach is a fresh decision to leave
// `127.0.0.1` (`RNS/Interfaces/LocalInterface.py:109`). Helpers come from the sibling
// `tests_parts` files included into this module; nothing answers on port 1 of either
// address.

/// The IPv6 loopback: accepted, never warned about.
const SHARED_INSTANCE_V6_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 1);

/// `10.255.255.1` written as an IPv4-mapped IPv6 address: not loopback in either notation.
const SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0aff, 0xff01)), 1);

/// Attaches `endpoint` with default options, detaches and stops; returns the loud records.
#[deny(clippy::await_holding_invalid_type)]
async fn probe12_attach_detach_stop(name: &str, endpoint: SocketAddr) -> Vec<(log::Level, String)> {
    install_test_logger();
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(endpoint),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("a loopback TCP endpoint of either family is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .collect()
}

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_the_ipv6_loopback_never_warns() {
    let loud = probe12_attach_detach_stop(
        "shared-instance-endpoint-v6-loopback",
        SHARED_INSTANCE_V6_LOOPBACK_ENDPOINT,
    )
    .await;
    assert!(loud.is_empty(), "[::1] is loopback, so no warning is due: {loud:?}");
}

/// The loopback test runs on the canonical form of the address, so the mapped notation of
/// a non-loopback address is refused like its IPv4 form, with the one refusal warning.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_a_mapped_non_loopback_address_is_refused_by_default() {
    install_test_logger();
    let name = "shared-instance-endpoint-v6-mapped";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let refused = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await;
    assert!(
        matches!(refused, Err(RnsError::SharedInstanceEndpointNotLoopback)),
        "a non-loopback address in any notation is refused: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), None, "a refused attach takes no slot");
    let loud: Vec<_> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .collect();
    assert_eq!(loud.len(), 1, "one warning for a refused non-loopback attach: {loud:?}");
    assert!(loud[0].1.contains("not a loopback address"), "says why: {}", loud[0].1);
    assert!(loud[0].1.contains("attach refused"), "says what happened: {}", loud[0].1);
    assert!(
        loud[0].1.contains(&SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT.to_string()),
        "names the endpoint as given: {}",
        loud[0].1
    );
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}

/// Two attaches under the opt-in, a detach between them: two warnings, one per attach,
/// each naming the endpoint; the detach and the stop add none.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_reattach_to_a_non_loopback_endpoint_warns_once_per_attach() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn-reattach";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    for attach in 1..=2u32 {
        let iface = transport
            .attach_shared_instance(
                SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
                SharedInstanceOptions::default().allow_remote_endpoint(true),
            )
            .await
            .expect("a non-loopback endpoint is accepted under the opt-in");
        shared_instance_assert_attached(&transport, iface).await;
        let warned = shared_instance_records_for(name)
            .into_iter()
            .filter(|(level, _)| *level == log::Level::Warn)
            .count();
        assert_eq!(warned, attach as usize, "attach {attach} adds exactly one warning");
        assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
        shared_instance_assert_released(&transport, iface).await;
    }
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    let loud = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .count();
    assert_eq!(loud, 2, "the detaches and the stop add no warning");
}
