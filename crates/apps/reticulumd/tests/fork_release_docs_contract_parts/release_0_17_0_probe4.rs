// Two ledger pins over the 0.17.0 release docs. The PATCHSET `release 0.17.0`
// narrative names test functions in backticks
// (`tcp_client_keeps_serving_when_the_connect_receiver_is_gone`,
// `connect_events_api_pins_carry_what_the_three_ledgers_describe`, ...); a
// consumer following the ledger to a test must find it, so every backticked
// snake_case name of five or more segments in that narrative (from the "Fork
// commits" paragraph that opens it to the end of the 0.17.0 section) must be
// a `fn` somewhere under `crates/`. The same narrative describes the
// connect-event pins as TCP tests "and their unix twins"; a count pin alone is
// blind to a swap that keeps the count but breaks the pairing, so the
// connect-event test files must pair every `tcp_client_<x>` test with a
// `local_unix_client_<x>` twin and vice versa.

use std::path::Path;

/// The `release 0.17.0` narrative: from the paragraph that opens with the
/// patch's fork-commit list to the end of the 0.17.0 PATCHSET section.
fn release_0_17_0_narrative() -> String {
    let patchset = read_repo_file("docs/fork/PATCHSET.md");
    let section_start =
        patchset.find("\n## 0.17.0 (upstream ").expect("PATCHSET has a 0.17.0 section");
    let section = &patchset[section_start + 1..];
    let section_end = section[3..].find("\n## ").map(|i| i + 3).unwrap_or(section.len());
    let section = &section[..section_end];
    let narrative_start = section
        .find("\nFork commits `972318f6`")
        .expect("the 0.17.0 section narrates the `release 0.17.0` patch by its fork commits");
    section[narrative_start..].to_string()
}

/// Every backticked span of `text` shaped like a snake_case identifier of at
/// least `min_segments` underscore-separated segments.
fn backticked_snake_case_names(text: &str, min_segments: usize) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        let shape = !token.is_empty()
            && token.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && token.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && token.split('_').count() >= min_segments;
        if shape {
            names.insert(token.to_string());
        }
        rest = &after[close + 1..];
    }
    names
}

/// Every `.rs` file under `dir`, `target/` directories skipped.
fn rust_files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read_dir {}: {err}", dir.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// The name of every `fn` item declared under `crates/`, read from the line
/// that declares it.
fn fn_names_under_crates() -> BTreeSet<String> {
    let mut files = Vec::new();
    rust_files_under(&repo_root().join("crates"), &mut files);
    let mut names = BTreeSet::new();
    for file in files {
        let Ok(source) = fs::read_to_string(&file) else { continue };
        for line in source.lines() {
            let line = line.trim_start();
            for prefix in ["fn ", "async fn ", "pub fn ", "pub async fn ", "pub(crate) fn "] {
                if let Some(rest) = line.strip_prefix(prefix) {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        names.insert(name);
                    }
                }
            }
        }
    }
    names
}

/// Every test function the `release 0.17.0` narrative names exists under
/// `crates/`. Five-plus underscore segments is the shape every test name in
/// that narrative has and no API identifier it cites does.
#[test]
fn release_0_17_0_narrative_names_only_test_functions_that_exist() {
    let narrative = release_0_17_0_narrative();
    let named = backticked_snake_case_names(&narrative, 5);
    assert!(
        named.contains("tcp_client_keeps_serving_when_the_connect_receiver_is_gone")
            && named.contains("connect_events_api_pins_carry_what_the_three_ledgers_describe"),
        "the narrative should name the connect-event pins, found {named:?}"
    );
    let defined = fn_names_under_crates();
    let missing: Vec<_> = named.iter().filter(|n| !defined.contains(*n)).collect();
    assert!(
        missing.is_empty(),
        "the release 0.17.0 narrative names test functions that do not exist:\n{missing:#?}"
    );
}

/// The names of the `async fn` items in a test source file.
fn test_fns(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("async fn "))
        .map(|rest| {
            rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect::<String>()
        })
        .collect()
}

/// In both connect-event test files every TCP test has a unix twin of the
/// same name and vice versa, every `#[tokio::test]` is one half of a pair,
/// the pairs number three in `tests/connect_events_api.rs` and two in
/// `tests/connect_events_log_api.rs`, and the narrative's two named TCP tests
/// are among the first file's.
#[test]
fn connect_event_tcp_pins_each_have_a_unix_twin() {
    for (file, pairs) in [
        ("crates/libs/rns-transport/tests/connect_events_api.rs", 3),
        ("crates/libs/rns-transport/tests/connect_events_log_api.rs", 2),
    ] {
        let source = read_repo_file(file);
        let fns = test_fns(&source);
        let tcp: BTreeSet<_> =
            fns.iter().filter_map(|f| f.strip_prefix("tcp_client_")).map(str::to_string).collect();
        let unix: BTreeSet<_> = fns
            .iter()
            .filter_map(|f| f.strip_prefix("local_unix_client_"))
            .map(str::to_string)
            .collect();
        assert_eq!(
            tcp, unix,
            "{file}: every tcp_client_* test needs a local_unix_client_* twin and vice versa"
        );
        assert_eq!(tcp.len(), pairs, "{file}: {pairs} twin pairs, got {tcp:?}");
        assert_eq!(
            tcp.len() + unix.len(),
            source.lines().filter(|l| l.trim() == "#[tokio::test]").count(),
            "{file}: every #[tokio::test] in the file is one half of a twin pair"
        );
    }

    let fns = test_fns(&read_repo_file("crates/libs/rns-transport/tests/connect_events_api.rs"));
    for narrated in [
        "tcp_client_keeps_serving_when_the_connect_receiver_is_gone",
        "tcp_client_drops_the_connect_event_on_a_full_channel_and_keeps_serving",
    ] {
        assert!(fns.contains(narrated), "the PATCHSET narrative names `{narrated}`");
        let twin = narrated.replacen("tcp_client_", "local_unix_client_", 1);
        assert!(fns.contains(&twin), "`{narrated}` has the unix twin `{twin}`");
    }
}
