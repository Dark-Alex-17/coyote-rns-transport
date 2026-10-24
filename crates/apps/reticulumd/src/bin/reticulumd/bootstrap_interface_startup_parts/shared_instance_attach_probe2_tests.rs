// Black-box pins for the strict-startup preflight order after the fix that dials the PICKED
// address (`resolve -> pick -> preflight`), from the consumer's side of the config: the
// default pick (no `prefer_ipv6`) on a `localhost` host whose resolver answers both
// families, a shared instance on the other loopback family, an IP-literal host under strict
// startup, and the non-strict path, which must not preflight at all. Helpers
// (`attach_test_args`, `test_transport`, `records_for`, `run_startup`,
// `assert_slot_holds_attached_record`, `runtime_field`) come from the parent test module.

use super::*;

/// Whether `localhost` answers with BOTH loopback families here. The default-pick test needs
/// the IPv4 answer for the pick and the IPv6 answer for its oracle to discriminate a
/// host-string preflight (which would connect over `[::1]` and pass); it skips with a note
/// on stderr where either is missing.
async fn localhost_resolves_to_both_families() -> bool {
    let addrs: Vec<SocketAddr> = match tokio::net::lookup_host("localhost:1").await {
        Ok(addrs) => addrs.collect(),
        Err(err) => {
            eprintln!("skipping: localhost does not resolve on this host: {err}");
            return false;
        }
    };
    let has_v4 = addrs.iter().any(|addr| addr.is_ipv4());
    let has_v6 = addrs.iter().any(|addr| addr.is_ipv6());
    if has_v4 && has_v6 {
        return true;
    }
    eprintln!("skipping: localhost answers v4={has_v4} v6={has_v6} on this host, need both");
    false
}

async fn bind_ipv6_loopback_listener() -> Option<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind("[::1]:0").await {
        Ok(listener) => Some(listener),
        Err(err) => {
            eprintln!("skipping: no IPv6 loopback on this host: {err}");
            None
        }
    }
}

/// The default-configuration mirror of the committed `prefer_ipv6` pin: without the knob the
/// pick is the IPv4 loopback, so a shared instance listening on `[::1]` alone is a recorded
/// strict-startup failure naming `127.0.0.1:<port>`. A preflight on the host string would
/// have connected over `[::1]`, passed, and left an attach that dials `127.0.0.1` forever.
#[tokio::test]
async fn strict_startup_default_pick_rejects_an_ipv6_only_listener_for_localhost() {
    if !localhost_resolves_to_both_families().await {
        return;
    }
    let Some(shared_listener) = bind_ipv6_loopback_listener().await else {
        return;
    };
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "strict-default-pick", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port} }}
]
"#
    ))
    .expect("a local_client entry without prefer_ipv6 parses");
    assert_eq!(
        config.interfaces[0].prefer_ipv6, None,
        "the knob is absent, the default order applies"
    );
    let mut args = attach_test_args();
    args.strict_interface_startup = true;
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 0, "the IPv6-only listener is not the default pick");
    assert_eq!(batch.startup_failures.len(), 1, "failures={:?}", batch.startup_failures);
    let failure = &batch.startup_failures[0];
    assert_eq!(failure.label, "strict-default-pick");
    assert!(
        failure.error.contains("preflight connect failed")
            || failure.error.contains("preflight connect timed out"),
        "the preflight is what failed: {}",
        failure.error
    );
    assert!(
        failure.error.contains(&format!("127.0.0.1:{port}")),
        "the failure names the picked IPv4 loopback: {}",
        failure.error
    );
    assert!(
        !failure.error.contains(&format!("localhost:{port}")),
        "the failure names the picked address, not the host string: {}",
        failure.error
    );
    assert!(transport.shared_instance().is_none(), "nothing was attached");
    assert_eq!(runtime_field(&records[0], "startup_status"), Some("failed"));
    // The preflight never dialled the IPv6 listener: nothing is waiting to be accepted.
    let dialled =
        tokio::time::timeout(std::time::Duration::from_millis(300), shared_listener.accept()).await;
    assert!(
        dialled.is_err(),
        "no connection may reach the IPv6-only listener under the default pick"
    );
}

/// An IP-literal host resolves to itself and the strict preflight dials that same address:
/// the reorder keeps the plain `host = "127.0.0.1"` strict path passing and attaching.
#[tokio::test]
async fn strict_startup_passes_for_an_ip_literal_host_and_attaches() {
    let shared_listener =
        tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "strict-ip-literal", shared_instance_type = "tcp", host = "127.0.0.1", shared_instance_port = {port} }}
]
"#
    ))
    .expect("a local_client entry with an IP-literal host parses");
    let mut args = attach_test_args();
    args.strict_interface_startup = true;
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1);
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    let (_stream, peer) =
        tokio::time::timeout(std::time::Duration::from_secs(5), shared_listener.accept())
            .await
            .expect("the preflight or the client dials the IPv4 loopback listener within 5 s")
            .expect("accept a connection");
    assert!(
        peer.ip().is_loopback() && peer.is_ipv4(),
        "dialled from the IPv4 loopback, not {peer}"
    );
    assert_slot_holds_attached_record(&transport, &records[0]).await;
}

/// Without `--strict-interface-startup` there is no preflight: an entry whose picked family
/// (`[::1]`, via `prefer_ipv6`) has no listener still attaches, the client retries in the
/// background, no startup failure is recorded, and the IPv4-only listener the attach never
/// dials sees no connection.
#[tokio::test]
async fn non_strict_startup_attaches_without_preflighting_the_picked_family() {
    if !localhost_resolves_to_both_families().await {
        return;
    }
    let shared_listener =
        tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind shared local instance");
    let port = shared_listener.local_addr().expect("shared local addr").port();
    let config = DaemonConfig::from_toml(&format!(
        r#"
interfaces = [
  {{ type = "LocalClientInterface", enabled = true, name = "lenient-other-family", shared_instance_type = "tcp", host = "localhost", shared_instance_port = {port}, prefer_ipv6 = true }}
]
"#
    ))
    .expect("prefer_ipv6 parses on a local_client entry");
    let args = attach_test_args();
    assert!(!args.strict_interface_startup, "the lenient path is the one under test");
    let transport = test_transport();
    let mut records = records_for(&config);

    let batch = run_startup(&args, &config, &transport, &mut records).await;

    assert_eq!(batch.startup_successes, 1, "a lenient start attaches without dialling first");
    assert!(batch.startup_failures.is_empty(), "failures={:?}", batch.startup_failures);
    assert_slot_holds_attached_record(&transport, &records[0]).await;
    let dialled =
        tokio::time::timeout(std::time::Duration::from_millis(500), shared_listener.accept()).await;
    assert!(
        dialled.is_err(),
        "the attach dials the picked [::1] only; the IPv4 listener must see no connection"
    );
}
