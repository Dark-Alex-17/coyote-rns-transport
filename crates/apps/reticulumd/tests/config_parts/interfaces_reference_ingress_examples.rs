/// The value of the commented `# <key> = <value>` example line in the reference TOML, the
/// remainder cut at the first `#` so an inline comment cannot reach the parser.
fn commented_example_value(key: &str) -> f64 {
    let input = include_str!("../../examples/interfaces-reference.toml");
    let prefix = format!("# {key} = ");
    let raw = input
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap_or_else(|| panic!("the reference TOML has no commented {key} line"));
    let raw = raw.split('#').next().unwrap_or_default().trim();
    raw.parse().unwrap_or_else(|err| panic!("{key}: {raw:?} does not parse: {err}"))
}

/// The commented ingress-control examples of the reference TOML read RNS 1.5.2's defaults,
/// the ones `coyote-rns-transport` applies when a `[[interfaces]]` block leaves them unset
/// (`RNS/Interfaces/Interface.py:72`, `:76-86`): the announce limiter's `ic_*` keys, the
/// path-request limiter's `ic_pr_*` keys and the egress `ec_pr_freq` key.
#[test]
fn reference_toml_ingress_control_examples_read_the_rns_1_5_2_defaults() {
    for (key, expected) in [
        ("ic_max_held_announces", 256.0),
        ("ic_burst_hold", 15.0),
        ("ic_burst_freq_new", 3.0),
        ("ic_burst_freq", 10.0),
        ("ic_pr_burst_freq_new", 3.0),
        ("ic_pr_burst_freq", 8.0),
        ("ec_pr_freq", 5.0),
        ("ic_new_time", 7200.0),
        ("ic_burst_penalty", 15.0),
        ("ic_held_release_interval", 5.0),
    ] {
        assert_eq!(commented_example_value(key), expected, "{key}");
    }
}
