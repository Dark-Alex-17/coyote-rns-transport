// Spec-first pin for the lxmd smoke script's port defaults: every loopback
// port the script picks when the caller has not exported one comes from a
// single Python helper that binds all of its sockets before reporting any, so
// concurrent runs on one host share no pid-derived range and the two Python
// control ports are no longer offsets of the shared-instance port. The test
// fails against the script as it was when the defaults were derived from the
// shell's pid.

const LXMD_SMOKE_SCRIPT: &str = "tools/scripts/python-lxmd-rust-lxmd-smoke.sh";

const LXMD_SMOKE_HELPER_DEFINITION: &str = "free_loopback_ports() {";

/// The bind loop closes before the one print that reports every port, so a
/// helper that binds, prints and closes per socket does not match.
const LXMD_SMOKE_BIND_ALL_THEN_REPORT: &str =
    "    for sock in sockets:\n        sock.bind((\"127.0.0.1\", 0))\n    print(";

const LXMD_SMOKE_PORT_VARIABLES: [&str; 5] = [
    "RUST_RPC_ADDR",
    "RUST_TRANSPORT_ADDR",
    "PY_SHARED_INSTANCE_PORT",
    "PY_INSTANCE_CONTROL_PORT",
    "PY_ENDPOINT_CONTROL_PORT",
];

/// The default of the `VAR="${VAR:-<default>}"` line for `variable`, the text
/// between `:-` and the closing brace of the outer expansion.
fn lxmd_smoke_env_default<'a>(script: &'a str, variable: &str) -> &'a str {
    let prefix = format!("{variable}=\"${{{variable}:-");
    let line = script
        .lines()
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("the smoke script keeps the {variable} env override"));
    line[prefix.len()..]
        .strip_suffix("}\"")
        .unwrap_or_else(|| panic!("the {variable} default line closes its expansion, got {line:?}"))
}

/// The `FREE_PORT_n` name a default reads.
fn lxmd_smoke_free_port_token(default: &str) -> &str {
    let at = default
        .find("${FREE_PORT_")
        .unwrap_or_else(|| panic!("the default reads a FREE_PORT_n the helper filled in, got {default:?}"));
    let token = &default[at + 2..];
    &token[..token.find('}').expect("the FREE_PORT_n expansion closes")]
}

/// The smoke script asks the OS for its five default loopback ports through
/// exactly one helper invocation that binds every socket before printing any,
/// each default reads a distinct port from that invocation, no default is
/// derived from the pid or from another port variable, and an exported value
/// still wins.
#[test]
fn lxmd_smoke_script_asks_the_os_for_its_loopback_ports() {
    let script = read_repo_file(LXMD_SMOKE_SCRIPT);
    assert!(!script.contains("$$ %"), "no port default is derived from the shell's pid");

    assert_eq!(
        script.matches(LXMD_SMOKE_HELPER_DEFINITION).count(),
        1,
        "the script defines free_loopback_ports once"
    );
    let helper_at = script.find(LXMD_SMOKE_HELPER_DEFINITION).expect("the script defines free_loopback_ports");
    let helper = &script[helper_at..];
    let helper = &helper[..helper.find("\nPY\n").expect("the helper's heredoc closes")];
    assert!(helper.contains("count = int(sys.argv[1])"), "the helper takes its socket count from argv");
    let bind_at = helper.find("sock.bind((\"127.0.0.1\", 0))").expect("the helper binds loopback port 0");
    let report_at = helper.find("getsockname()[1]").expect("the helper reports the bound port");
    assert!(bind_at < report_at, "the helper binds every socket before it reports the first port");
    assert!(
        helper.contains(LXMD_SMOKE_BIND_ALL_THEN_REPORT),
        "the helper's bind loop finishes before its one print, got {helper:?}"
    );

    let helper_call = format!("free_loopback_ports {})", LXMD_SMOKE_PORT_VARIABLES.len());
    assert_eq!(
        script.matches(&helper_call).count(),
        1,
        "the script calls the helper once per run, asking for one port per default"
    );
    let read_line = script
        .lines()
        .find(|line| line.starts_with("read -r ") && line.contains(&helper_call))
        .expect("the script reads the helper's output into FREE_PORT_n names");

    let mut tokens = Vec::new();
    for variable in LXMD_SMOKE_PORT_VARIABLES {
        let default = lxmd_smoke_env_default(&script, variable);
        for other in LXMD_SMOKE_PORT_VARIABLES.iter().filter(|other| **other != variable) {
            assert!(!default.contains(other), "the {variable} default is not an offset of {other}, got {default:?}");
        }
        let token = lxmd_smoke_free_port_token(default);
        assert!(read_line.contains(&format!(" {token} ")), "{token} is filled by the read line, got {read_line:?}");
        tokens.push(token);
    }
    tokens.sort_unstable();
    tokens.dedup();
    assert_eq!(tokens.len(), LXMD_SMOKE_PORT_VARIABLES.len(), "the five defaults read five distinct ports");
}
