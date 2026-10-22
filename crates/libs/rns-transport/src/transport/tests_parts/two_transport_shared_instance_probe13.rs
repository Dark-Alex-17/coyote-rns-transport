// Pins the two edges of the non-loopback endpoint warning that the attach rustdoc states
// and `two_transport_shared_instance_endpoint_warn.rs` / `_probe12.rs` leave open, read
// through the same record store: the loopback test runs on the canonical form of the
// address, so `127.0.0.1` written as the IPv4-mapped IPv6 address `[::ffff:127.0.0.1]` is
// as quiet as its IPv4 form (Python's `LocalClientInterface` dials that very address,
// `RNS/Interfaces/LocalInterface.py:109`); and an attach refused because the transport has
// stopped warns nothing even for a non-loopback endpoint, the same silence the slot-held
// refusal keeps. Helpers come from the sibling `tests_parts` files included into this
// module; nothing answers on port 1 of the mapped address.

/// `127.0.0.1` written as an IPv4-mapped IPv6 address: loopback once canonicalised.
const SHARED_INSTANCE_V6_MAPPED_LOOPBACK_ENDPOINT: SocketAddr =
    SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x0001)), 1);

#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_to_the_mapped_ipv4_loopback_never_warns() {
    let loud = probe12_attach_detach_stop(
        "shared-instance-endpoint-v6-mapped-loopback",
        SHARED_INSTANCE_V6_MAPPED_LOOPBACK_ENDPOINT,
    )
    .await;
    assert!(
        loud.is_empty(),
        "[::ffff:127.0.0.1] is loopback in canonical form, so no warning is due: {loud:?}"
    );
}

/// After `stop` the attach is refused under the slot lock, before the interface is spawned
/// and before the warning site is reached: the refusal is `InvalidArgument`, the posture
/// stays clear and the record store holds no warning at all.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_after_stop_to_a_non_loopback_endpoint_does_not_warn() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn-after-stop";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
    let refused = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await;
    assert!(
        matches!(refused, Err(RnsError::InvalidArgument)),
        "an attach after stop is refused: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), None, "a refused attach takes no slot");
    assert!(!transport.connected_to_shared_instance().await, "the posture stays clear");
    let loud: Vec<_> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .collect();
    assert!(loud.is_empty(), "a refused attach warns nothing, loopback or not: {loud:?}");
}
