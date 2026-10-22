// Pins the interface token on the non-loopback endpoint warning: the one warning an
// accepted TCP attach to a non-loopback address emits names the interface the attach
// returned as a whole `iface=<hash>` token, placed before the `endpoint <addr>` clause, and
// the token is the same one the attach debug line carries, so a reader can join the two
// lines on it. A second attach after a detach warns with its own interface, not the
// first one's. The address, the cite and the "proceeds" clause are pinned by
// `two_transport_shared_instance_endpoint_warn.rs`; the attach debug line's token shape by
// `two_transport_shared_instance_probe6.rs`. Helpers come from the sibling `tests_parts`
// files included into this module.

/// The tokens after `shared instance ` on the one warning recorded for `transport_name`,
/// split on single spaces. Fails when there is not exactly one warning.
fn shared_instance_endpoint_warning_tokens(transport_name: &str) -> Vec<String> {
    let warned: Vec<String> = shared_instance_records_for(transport_name)
        .into_iter()
        .filter(|(level, _)| *level == log::Level::Warn)
        .map(|(_, record)| record)
        .collect();
    assert_eq!(warned.len(), 1, "one warning per non-loopback attach: {warned:?}");
    let line = &warned[0];
    let (_, fields) = line.split_once("shared instance ").expect("the warning carries its fields");
    fields.split(' ').map(str::to_owned).collect()
}

/// The `iface=` token on the one attach debug line recorded for `transport_name`.
fn shared_instance_attach_line_iface_token(transport_name: &str) -> String {
    let attached =
        shared_instance_debug_records_containing(transport_name, "attached shared instance ");
    assert_eq!(attached.len(), 1, "one attach line per attach: {attached:?}");
    let (_, fields) = attached[0]
        .split_once("attached shared instance ")
        .expect("the attach line carries its fields");
    fields.split(' ').next().expect("the attach line has a first token").to_owned()
}

/// The warning reads `shared instance iface=<hash> endpoint <addr> is not a loopback
/// address ...`: the interface token is the attach's return value, whole, and sits before
/// the endpoint clause; it equals the attach debug line's `iface=` token.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_non_loopback_warning_names_the_attached_interface_as_a_token() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn-iface-token";
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

    let tokens = shared_instance_endpoint_warning_tokens(name);
    assert!(tokens.len() >= 3, "iface token, endpoint word and address: {tokens:?}");
    assert_eq!(
        tokens[0],
        format!("iface={iface}"),
        "the first token after `shared instance` names the attached interface: {tokens:?}"
    );
    assert_eq!(tokens[1], "endpoint", "the endpoint clause follows the interface: {tokens:?}");
    assert_eq!(
        tokens[2],
        SHARED_INSTANCE_UNREACHABLE_ENDPOINT.to_string(),
        "the endpoint word is followed by the address: {tokens:?}"
    );
    assert_eq!(
        tokens[0],
        shared_instance_attach_line_iface_token(name),
        "the warning and the attach debug line carry the same interface token"
    );

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}

/// After a detach, a second non-loopback attach warns with the interface it returned,
/// not the first attach's: the token is per-attach.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_non_loopback_warning_names_each_attach_own_interface() {
    install_test_logger();
    let name = "shared-instance-endpoint-warn-iface-token-reattach";
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let first = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the first non-loopback attach is accepted");
    shared_instance_assert_attached(&transport, first).await;
    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, first).await;

    let second = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Tcp(SHARED_INSTANCE_UNREACHABLE_ENDPOINT),
            SharedInstanceOptions::default(),
        )
        .await
        .expect("the second non-loopback attach is accepted");
    shared_instance_assert_attached(&transport, second).await;
    assert_ne!(first, second, "each attach gets its own interface");

    let iface_tokens: Vec<String> = shared_instance_records_for(name)
        .into_iter()
        .filter(|(level, record)| {
            *level == log::Level::Warn && record.contains("not a loopback address")
        })
        .map(|(_, record)| {
            let (_, fields) =
                record.split_once("shared instance ").expect("the warning carries its fields");
            fields.split(' ').next().expect("a first token").to_owned()
        })
        .collect();
    assert_eq!(
        iface_tokens,
        vec![format!("iface={first}"), format!("iface={second}")],
        "one warning per attach, each naming its own interface"
    );

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, second).await;
    timeout(STOP_TIMEOUT, transport.stop()).await.expect("stop completes");
}
