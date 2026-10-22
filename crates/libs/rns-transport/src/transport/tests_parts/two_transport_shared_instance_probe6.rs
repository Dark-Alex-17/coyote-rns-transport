// Pins the shape of the attach debug line, not just its words: for a tcp and a unix
// endpoint alike it is space-separated `key=value` tokens (`iface=`, `endpoint=`,
// `reconnect_wait=`) with no comma separators, ending at the wait. A regression to the earlier
// `endpoint={}, reconnect_wait={:?} ignored ...` form keeps every word and fails here.
// Helpers come from the sibling `tests_parts` files included into this module.

/// The tokens after `attached shared instance ` on the one attach line recorded for
/// `transport_name`, split on single spaces, with the three leading `key=value` tokens
/// checked for shape and order.
fn shared_instance_attach_line_tokens(
    transport_name: &str,
    iface: &str,
    kind: &str,
) -> Vec<String> {
    let attached =
        shared_instance_debug_records_containing(transport_name, "attached shared instance ");
    assert_eq!(attached.len(), 1, "one attach line per attach: {attached:?}");
    let line = &attached[0];
    let (_, fields) =
        line.split_once("attached shared instance ").expect("the attach line carries its fields");
    assert!(
        !fields.contains(','),
        "key=value fields are space-separated, not comma-separated: {line}"
    );
    assert!(!fields.contains("  "), "fields are separated by single spaces: {line}");
    let tokens: Vec<String> = fields.split(' ').map(str::to_owned).collect();
    assert!(tokens.len() >= 3, "iface, endpoint and reconnect_wait tokens: {line}");
    assert_eq!(tokens[0], format!("iface={iface}"), "first token names the interface: {line}");
    assert_eq!(
        tokens[1],
        format!("endpoint={kind}"),
        "second token names the endpoint kind: {line}"
    );
    let wait = tokens[2]
        .strip_prefix("reconnect_wait=")
        .unwrap_or_else(|| panic!("third token is the reconnect wait as key=value: {line}"));
    assert_eq!(
        wait, "250ms",
        "the wait is the configured value, bare, with no trailing comma: {line}"
    );
    tokens
}

/// The TCP attach line is `iface=<hash> endpoint=tcp reconnect_wait=<wait>`: three
/// space-separated `key=value` tokens ending at the wait, nothing else, since the
/// `TcpClient` honours the wait.
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_tcp_attach_line_is_space_separated_key_value_and_ends_at_the_wait() {
    install_test_logger();
    let name = "shared-instance-log-shape-tcp";
    let server = shared_instance_server().await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            server.endpoint(),
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(250)),
        )
        .await
        .expect("attach to the shared instance");
    server.await_accepted_connections(1).await;
    shared_instance_assert_attached(&transport, iface).await;

    let tokens = shared_instance_attach_line_tokens(name, &iface.to_string(), "tcp");
    assert_eq!(tokens.len(), 3, "the tcp line ends at the wait: {tokens:?}");
    shared_instance_assert_never_warned(name);

    assert!(timeout(STOP_TIMEOUT, transport.detach_shared_instance()).await.expect("detach"));
    shared_instance_assert_released(&transport, iface).await;
    timeout(STOP_TIMEOUT, server.transport.stop()).await.expect("shared instance stop completes");
}

/// The unix attach line is the same three space-separated `key=value` tokens and stops at
/// the wait: the `LocalUnixClient` honours it, so no parenthetical follows.
#[cfg(unix)]
#[tokio::test]
#[deny(clippy::await_holding_invalid_type)]
async fn shared_instance_unix_attach_line_is_space_separated_key_value_and_ends_at_the_wait() {
    install_test_logger();
    let name = "shared-instance-log-shape-unix";
    let temp = tempfile::tempdir().expect("tempdir");
    let (owner, unix_endpoint) = shared_instance_unix_owner(temp.path()).await;
    let identity = PrivateIdentity::new_from_rand(OsRng);
    let transport = Transport::new(TransportConfig::new(name, &identity, false));
    let iface = transport
        .attach_shared_instance(
            SharedInstanceEndpoint::Unix(unix_endpoint),
            SharedInstanceOptions::default().with_reconnect_wait(Duration::from_millis(250)),
        )
        .await
        .expect("attach to the unix shared instance");
    shared_instance_assert_attached(&transport, iface).await;

    let tokens = shared_instance_attach_line_tokens(name, &iface.to_string(), "unix");
    assert_eq!(tokens.len(), 3, "the unix line ends at the wait: {tokens:?}");
    shared_instance_assert_never_warned(name);

    timeout(STOP_TIMEOUT, transport.stop()).await.expect("client stop completes");
    timeout(STOP_TIMEOUT, owner.stop()).await.expect("unix shared instance stop completes");
}
