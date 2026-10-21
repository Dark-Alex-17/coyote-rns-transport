// Pins the boundary of the non-loopback endpoint warning, read through the same
// record store as `two_transport_shared_instance_endpoint_warn.rs`: the warning keys on
// "loopback", not on `127.0.0.1`, so the IPv6 loopback `[::1]` is as quiet as the IPv4 one,
// an IPv4-mapped IPv6 address that is not loopback warns like its IPv4 form, and the
// warning is per attach: a detach followed by a second attach to the same endpoint warns
// again, since each attach is a fresh decision to leave `127.0.0.1`
// (`RNS/Interfaces/LocalInterface.py:109`). Helpers come from the sibling `tests_parts`
// files included into this module; nothing answers on port 1 of either address.

/// The IPv6 loopback: accepted, never warned about.
const SHARED_INSTANCE_V6_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 1);

/// `10.255.255.1` written as an IPv4-mapped IPv6 address: not loopback in either notation.
const SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x0aff, 0xff01)), 1);

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
        .expect("a TCP endpoint of either family is accepted");
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

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_a_mapped_non_loopback_address_warns_once() {
    let loud = probe12_attach_detach_stop(
        "shared-instance-endpoint-v6-mapped",
        SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT,
    )
    .await;
    assert_eq!(loud.len(), 1, "one warning for a non-loopback address in any notation: {loud:?}");
    assert!(loud[0].1.contains("not a loopback address"), "says why: {}", loud[0].1);
    assert!(
        loud[0].1.contains(&SHARED_INSTANCE_V6_MAPPED_NON_LOOPBACK_ENDPOINT.to_string()),
        "names the endpoint as given: {}",
        loud[0].1
    );
}

/// Two attaches, a detach between them: two warnings, one per attach, each naming the
/// endpoint; the detach and the stop add none.
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
                SharedInstanceOptions::default(),
            )
            .await
            .expect("a non-loopback endpoint is accepted");
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
