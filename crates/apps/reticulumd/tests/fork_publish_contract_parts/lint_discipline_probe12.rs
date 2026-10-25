// Usage pin on the announce entry point the apps use: `send_announce` only
// logs when an announce cannot leave, `try_send_announce` returns the error
// to the caller. Every app site was moved to the fallible form and reports
// the error in its own way, so no production source under `crates/apps/` may
// call the bare one again. The one bare call left is a test helper, named here.

/// The method call as it appears at a call site.
const BARE_ANNOUNCE_CALL: &str = ".send_announce(";

/// The sanctioned bare call: a test helper that announces a channel endpoint
/// for the Python interop runs and has no caller to report to.
const SANCTIONED_BARE_ANNOUNCE: &str =
    "crates/apps/reticulumd/tests/support/python_channel_events.rs";

/// No production source under `crates/apps/` calls `send_announce`: the apps
/// report an undelivered announce through `try_send_announce` and their own
/// warn text. The sanctioned test helper is pinned so the name here goes
/// with it.
#[test]
fn no_app_production_source_calls_the_bare_send_announce() {
    let mut offenders: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if !path.starts_with("crates/apps/") || is_test_or_example_path(&path) {
            continue;
        }
        let text = read_repo_file(&path);
        offenders.extend(
            text.lines()
                .enumerate()
                .filter(|(_, line)| line.contains(BARE_ANNOUNCE_CALL))
                .map(|(index, _)| format!("{path}:{}", index + 1)),
        );
    }
    assert!(
        offenders.is_empty(),
        "app production code calls the bare {BARE_ANNOUNCE_CALL} at {offenders:?}; use try_send_announce and warn on the error"
    );
    assert!(
        read_repo_file(SANCTIONED_BARE_ANNOUNCE).contains(BARE_ANNOUNCE_CALL),
        "{SANCTIONED_BARE_ANNOUNCE} is the one sanctioned bare call; drop this pin when it goes"
    );
}
