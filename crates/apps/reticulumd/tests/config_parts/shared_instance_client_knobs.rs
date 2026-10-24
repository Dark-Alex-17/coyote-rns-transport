// The shared-instance client knobs after the daemon's migration onto
// `attach_shared_instance`: `connect_timeout` and `max_reconnect_tries` still parse on the
// `local` and `local_client` kinds (the config surface is unchanged) and `connect_timeout = 0`
// is still refused, while the settings echo drops the two keys for those kinds and keeps
// them for `tcp_client`, and echoes `prefer_ipv6`, the knob the client does honour; the
// reference TOML's `local` and `local_client` blocks no longer advertise the two and do
// advertise `prefer_ipv6`. Pins the CHANGELOG's "the config still parses them and still
// rejects `connect_timeout = 0`" and "no longer echoed in settings JSON for those kinds".

fn local_kind_with_knobs(kind: &str) -> InterfaceConfig {
    let input = format!(
        r#"
interfaces = [
  {{ type = "{kind}", enabled = true, name = "knobs", shared_instance_type = "tcp", host = "127.0.0.1", shared_instance_port = 37428, connect_timeout = 5, max_reconnect_tries = 3, prefer_ipv6 = true }}
]
"#
    );
    let cfg = DaemonConfig::from_toml(&input)
        .unwrap_or_else(|err| panic!("{kind} with the client knobs still parses: {err}"));
    cfg.interfaces.into_iter().next().expect("one interface")
}

#[test]
fn local_kinds_still_parse_the_timeout_and_retry_knobs() {
    for (kind, expected_kind) in
        [("LocalClientInterface", "local_client"), ("LocalInterface", "local")]
    {
        let iface = local_kind_with_knobs(kind);
        assert_eq!(iface.kind, expected_kind);
        assert_eq!(iface.connect_timeout, Some(5), "{kind} keeps connect_timeout");
        assert_eq!(iface.max_reconnect_tries, Some(3), "{kind} keeps max_reconnect_tries");
        assert_eq!(iface.prefer_ipv6, Some(true), "{kind} keeps prefer_ipv6");
    }
}

#[test]
fn settings_echo_drops_the_timeout_and_retry_knobs_for_local_kinds_only() {
    for kind in ["LocalClientInterface", "LocalInterface"] {
        let iface = local_kind_with_knobs(kind);
        let settings = iface.settings_json().expect("local settings");
        let object = settings.as_object().expect("settings object");
        assert!(
            !object.contains_key("connect_timeout"),
            "{kind}: connect_timeout is not echoed: {settings}"
        );
        assert!(
            !object.contains_key("max_reconnect_tries"),
            "{kind}: max_reconnect_tries is not echoed: {settings}"
        );
        assert_eq!(settings["host"], "127.0.0.1", "{kind}: the host is still echoed");
        assert_eq!(settings["port"], 37_428, "{kind}: the port is still echoed");
        assert_eq!(settings["prefer_ipv6"], true, "{kind}: the honoured knob is echoed");
    }

    let input = r#"
interfaces = [
  { type = "tcp_client", enabled = true, name = "wan", host = "rmap.world", port = 4242, connect_timeout = 5, max_reconnect_tries = 3 }
]
"#;
    let cfg = DaemonConfig::from_toml(input).expect("tcp_client with the knobs");
    let settings = cfg.interfaces[0].settings_json().expect("tcp_client settings");
    assert_eq!(settings["connect_timeout"], 5, "tcp_client still echoes connect_timeout");
    assert_eq!(settings["max_reconnect_tries"], 3, "tcp_client still echoes max_reconnect_tries");
}

#[test]
fn local_kinds_still_refuse_a_zero_connect_timeout() {
    for kind in ["LocalClientInterface", "LocalInterface"] {
        let input = format!(
            r#"
interfaces = [
  {{ type = "{kind}", enabled = true, name = "zero", shared_instance_type = "tcp", host = "127.0.0.1", shared_instance_port = 37428, connect_timeout = 0 }}
]
"#
        );
        let err = DaemonConfig::from_toml(&input)
            .err()
            .unwrap_or_else(|| panic!("{kind} with connect_timeout = 0 must be refused"));
        assert!(
            err.to_string().contains("connect_timeout must be > 0 for local"),
            "{kind}: the refusal names the rule: {err}"
        );
    }
}

/// The text of one `[[interfaces]]` block of the reference TOML, from its `type = "<kind>"`
/// line to the next block header or the end of the file.
fn reference_block(kind: &str) -> String {
    let input = include_str!("../../examples/interfaces-reference.toml");
    let header = format!("type = \"{kind}\"");
    let start =
        input.find(&header).unwrap_or_else(|| panic!("the reference TOML has a {kind} block"));
    let rest = &input[start..];
    let end = rest.find("[[interfaces]]").unwrap_or(rest.len());
    rest[..end].to_string()
}

#[test]
fn reference_toml_local_blocks_advertise_prefer_ipv6_and_no_timeout_or_retry_knob() {
    for kind in ["local", "local_client"] {
        let block = reference_block(kind);
        for knob in ["connect_timeout", "max_reconnect_tries"] {
            assert!(
                !block.contains(knob),
                "the reference TOML's {kind} block no longer advertises {knob}:\n{block}"
            );
        }
        assert!(
            block.contains("prefer_ipv6"),
            "the reference TOML's {kind} block advertises prefer_ipv6:\n{block}"
        );
    }
    let tcp_client = reference_block("tcp_client");
    for knob in ["connect_timeout", "max_reconnect_tries"] {
        assert!(tcp_client.contains(knob), "the tcp_client block still advertises {knob}");
    }
}
