// Usage-pattern pins over the 0.17.0 release docs, read the way a consumer
// reads them: the 0.17.0 register of the crate README names the
// `ResponseOutcome` variants a `track_outcome` caller will match on, so that
// list must be the enum's list, payload included; the register and the
// CHANGELOG point the consumer at `tests/*.rs` files for the behaviour they
// promise, so every such file must exist under the crate; and
// `tests/connect_events_api.rs` must carry the six tests and the five call
// shapes the three ledgers describe.

use std::collections::BTreeSet;

/// The crate README's `## Patch list` section, whitespace-joined, cut to the
/// 0.17.0 block (from its `What \`0.17.0\`` marker to the 0.16.0 marker).
fn readme_0_17_0_block() -> String {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let patch_list = joined_prose(crate_readme_section(&readme, "Patch list"));
    let at_0_17_0 = patch_list
        .find("What `0.17.0` carries on top of `0.16.0`:")
        .expect("the patch list should carry the 0.17.0 block");
    let at_0_16_0 = patch_list
        .find("What `0.16.0` carries on top of `0.15.1`:")
        .expect("the patch list should carry the 0.16.0 block");
    patch_list[at_0_17_0..at_0_16_0].to_string()
}

/// The CHANGELOG's 0.17.0 release section.
fn changelog_0_17_0_section() -> String {
    changelog_release_section(&read_repo_file("crates/libs/rns-transport/CHANGELOG.md"), "0.17.0")
}

/// Every backticked span of `text`, in order.
fn backticked(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        spans.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    spans
}

/// The variant texts of `pub enum <name>` in a source file: each line of the
/// body that is not a doc comment or attribute, with its trailing comma
/// dropped (`Packet`, `Resource(OutboundResult)`).
fn enum_variants(source: &str, name: &str) -> BTreeSet<String> {
    let opener = format!("pub enum {name} {{");
    let start = source.find(&opener).unwrap_or_else(|| panic!("`{opener}` should exist"));
    let body = &source[start + opener.len()..];
    let end = body.find("\n}").expect("the enum body should close");
    body[..end]
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("///") && !line.starts_with("#["))
        .map(|line| line.trim_end_matches(',').to_string())
        .collect()
}

/// The 0.17.0 register tells a `track_outcome` caller the `ResponseOutcome` it
/// resolves with is "one of `Packet`, `Resource(OutboundResult)` or `NotSent`":
/// a consumer writes their match arms from that sentence, so the variants it
/// names (payload included) must be exactly the variants the enum declares.
/// A payload named after a neighbouring type (`OutboundOutcome` for
/// `OutboundResult`) is the kind of drift this catches for any of the three
/// names, not just the one the string needles pin.
#[test]
fn readme_0_17_0_register_names_exactly_the_response_outcome_variants() {
    let block = readme_0_17_0_block();
    let lead = "resolves with a `ResponseOutcome`, one of ";
    let at = block.find(lead).expect("the register should say what a `ResponseOutcome` is one of");
    let sentence = &block[at + lead.len()..];
    let sentence = &sentence[..sentence.find(". ").expect("the variant sentence should end")];
    let named: BTreeSet<String> = backticked(sentence).into_iter().collect();
    assert_eq!(named.len(), 3, "the register should name three variants, got {sentence:?}");

    let declared = enum_variants(
        &read_repo_file("crates/libs/rns-transport/src/destination/request/outcome.rs"),
        "ResponseOutcome",
    );
    assert_eq!(
        named, declared,
        "the register's `ResponseOutcome` variants should be exactly the enum's (payload included)"
    );
}

/// Both release ledgers point the consumer at `tests/<name>.rs` files for the
/// behaviour they promise (`tests/connect_events_api.rs` among them). A
/// consumer who opens the crate must find every one of them; a renamed or
/// mistyped path is a dead pointer the string needles cannot see.
#[test]
fn readme_0_17_0_register_and_changelog_0_17_0_point_at_test_files_that_exist() {
    let mut pointers: Vec<(&str, String)> = Vec::new();
    for span in backticked(&readme_0_17_0_block()) {
        if span.starts_with("tests/") && span.ends_with(".rs") {
            pointers.push(("README 0.17.0 register", span));
        }
    }
    for span in backticked(&joined_prose(&changelog_0_17_0_section())) {
        if span.starts_with("tests/") && span.ends_with(".rs") {
            pointers.push(("CHANGELOG 0.17.0", span));
        }
    }
    assert!(
        pointers.iter().any(|(_, p)| p == "tests/connect_events_api.rs"),
        "the 0.17.0 docs should point at tests/connect_events_api.rs, found {pointers:?}"
    );
    let crate_root = repo_root().join("crates/libs/rns-transport");
    let missing: Vec<_> = pointers
        .iter()
        .filter(|(_, path)| !crate_root.join(path).is_file())
        .map(|(doc, path)| format!("{doc} names `{path}`"))
        .collect();
    assert!(
        missing.is_empty(),
        "every `tests/*.rs` the 0.17.0 docs name should exist:\n{}",
        missing.join("\n")
    );
}

/// The CHANGELOG `### Added` bullet and the PATCHSET 0.17.0 narrative say
/// `tests/connect_events_api.rs` carries six tests: the two register pins
/// that drive `TcpClient::with_connect_events` and
/// `LocalUnixClient::with_connect_events` from outside the crate, read the
/// `connect_events_enabled` getter and watch the reconnect channel beside the
/// connect channel, and the four dropped- or full-channel pins; the README
/// register names the file without a count. The file must carry the six
/// tests and the five call shapes the claim rests on.
#[test]
fn connect_events_api_pins_carry_what_the_three_ledgers_describe() {
    let source = read_repo_file("crates/libs/rns-transport/tests/connect_events_api.rs");
    let tests = source.lines().filter(|line| line.trim() == "#[tokio::test]").count();
    assert_eq!(tests, 6, "the ledgers describe six tests");
    for shape in [
        "TcpClient::new(",
        "LocalUnixClient::new_connect(",
        ".with_connect_events(",
        ".with_reconnect_events(",
        ".connect_events_enabled()",
    ] {
        assert!(source.contains(shape), "tests/connect_events_api.rs should call `{shape}`");
    }
    let changelog = changelog_0_17_0_section();
    let patchset = read_repo_file("docs/fork/PATCHSET.md");
    let heading = "\n## 0.17.0 (upstream 0.14.0)\n";
    let start = patchset.find(heading).expect("PATCHSET should carry the 0.17.0 heading");
    let body = &patchset[start + heading.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    let patchset_0_17_0 = &body[..end];
    for (doc, text) in
        [("CHANGELOG 0.17.0", changelog.as_str()), ("PATCHSET 0.17.0", patchset_0_17_0)]
    {
        let joined = joined_prose(text);
        assert!(
            joined.contains("connect_events_api.rs") && joined.contains("reconnect channel"),
            "{doc} should describe tests/connect_events_api.rs against the reconnect channel"
        );
    }
}
