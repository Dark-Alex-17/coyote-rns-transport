// Usage probe of the loopback rule's ordering claim: the CHANGELOG and the README say a
// non-loopback `Tcp` endpoint is refused "before any lock is taken, with one warning". A
// consumer reads that as: the loopback check comes before the slot and before the stop
// posture, so the refusal is `SharedInstanceEndpointNotLoopback` even when the slot is
// already held or the transport is stopped, and each such refusal warns exactly once, the
// warning naming the endpoint, the refusal and `RNS/Interfaces/LocalInterface.py:109`.
// The two sibling orderings are pinned elsewhere: a held slot refuses an opted-in remote
// endpoint as `InvalidArgument` without a warning
// (`two_transport_shared_instance_endpoint_warn.rs`) and so does a stopped transport
// (`two_transport_shared_instance_probe13.rs`). Helpers come from the sibling
// `tests_parts` files included into this module.

/// The warning each refused non-loopback attach is promised to leave.
fn shared_instance_assert_one_refusal_warning(transport_name: &str, endpoint: SocketAddr) {
    let warned: Vec<String> = shared_instance_records_for(transport_name)
        .into_iter()
        .filter(|(level, _)| *level <= log::Level::Warn)
        .map(|(_, record)| record)
        .collect();
    assert_eq!(warned.len(), 1, "one warning per refused attach: {warned:?}");
    let line = &warned[0];
    assert!(line.contains(&endpoint.to_string()), "names the endpoint: {line}");
    assert!(line.contains("not a loopback address"), "says why: {line}");
    assert!(line.contains("attach refused"), "says what happened: {line}");
    assert!(line.contains("RNS/Interfaces/LocalInterface.py:109"), "cites Python: {line}");
}

/// With the slot already held by a loopback attach, a non-loopback attach without the
/// opt-in is refused for the loopback rule, not for the held slot: the error is
/// `SharedInstanceEndpointNotLoopback`, the first attach keeps the slot and the posture,
/// and the refusal warns once while the loopback attach before it warned nothing.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_non_loopback_refusal_comes_before_the_held_slot() {
    install_test_logger();
    let name = "shared-instance-probe15-held-slot";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_V4_LOOPBACK_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("a loopback attach is accepted");
    shared_instance_assert_attached(&transport, iface).await;
    assert!(
        shared_instance_records_for(name).iter().all(|(level, _)| *level > log::Level::Warn),
        "a loopback attach warns nothing"
    );
    let interfaces_before = transport.interface_hashes().await;

    let refused = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await;
    assert!(
        matches!(refused, Err(RnsError::SharedInstanceEndpointNotLoopback)),
        "the loopback rule is checked before the slot: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), Some(iface), "the first attach keeps the slot");
    assert!(transport.connected_to_shared_instance().await, "the posture is untouched");
    assert_eq!(
        transport.interface_hashes().await,
        interfaces_before,
        "a refused attach registers no interface"
    );
    shared_instance_assert_one_refusal_warning(name, SHARED_INSTANCE_UNREACHABLE_ENDPOINT);

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}

/// On a stopped transport a non-loopback attach without the opt-in is still refused for
/// the loopback rule, with its one warning: the pre-lock check does not know about the
/// stop, and the stopped transport takes no slot and sets no posture.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_non_loopback_refusal_comes_before_the_stop_posture() {
    install_test_logger();
    let name = "shared-instance-probe15-after-stop";
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
        matches!(refused, Err(RnsError::SharedInstanceEndpointNotLoopback)),
        "the loopback rule is checked before the stop posture: {refused:?}"
    );
    assert_eq!(transport.shared_instance(), None, "a refused attach takes no slot");
    assert!(!transport.connected_to_shared_instance().await, "the posture stays clear");
    shared_instance_assert_one_refusal_warning(name, SHARED_INSTANCE_UNREACHABLE_ENDPOINT);
}

/// The refusal error reads as the rule it enforces, so a consumer logging `{err}` sees why
/// the attach failed without matching on the variant.
#[test]
fn shared_instance_not_loopback_error_names_the_rule_in_its_display() {
    let text = RnsError::SharedInstanceEndpointNotLoopback.to_string();
    assert!(text.contains("shared instance"), "names the subject: {text}");
    assert!(text.contains("loopback"), "names the rule: {text}");
}
