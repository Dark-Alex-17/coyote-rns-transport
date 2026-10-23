// Usage pins for the guard discipline: the `clippy::await_holding_invalid_type`
// lint runs at its default level under the workspace clippy invocation, no
// lint table mentions it, every production allowance is fn-level and names
// its owner on the same line, test roots allow it once under a comment that
// says why, and the lock discipline the transport markers point at is
// documented on `TransportHandler`.
//
// The needles are assembled with `concat!` and `format!` so this file never
// contains the attribute or marker text it scans for.

/// The lint name as it appears in lint tables and attributes.
const GUARD_LINT: &str = "await_holding_invalid_type";

/// A fn-level allowance: `#[allow(clippy::<lint>)]`.
fn fn_level_allow() -> String {
    format!("#[allow(clippy::{GUARD_LINT})]")
}

/// A module-level allowance: `#![allow(clippy::<lint>)]`.
fn module_level_allow() -> String {
    format!("#![allow(clippy::{GUARD_LINT})]")
}

/// The owner tag every production allowance must carry on the same line.
const OWNER_TAG: &str = concat!("// TASK-", "026:");

/// The bare owner tag: rns-transport sites carry it followed by a reason of
/// their own.
fn owner_tag() -> String {
    OWNER_TAG.to_string()
}

/// The marker on the transport-layer sites where the handler guard spans the
/// whole job by design.
fn transport_marker() -> String {
    format!(
        "{OWNER_TAG} handler guard spans the job by design, see TransportHandler lock discipline"
    )
}

/// The exact marker on the app-crate sites that still await a restructure.
fn app_crate_marker() -> String {
    format!("{OWNER_TAG} restructure this guard-across-await site, then remove")
}

/// The reason every test or example root states in the comment directly
/// above its allowance.
const TEST_ROOT_REASON: &str = "holds guards across awaits on purpose";

/// Production sources whose allowances this probe pins, with the marker text
/// each must carry on the same line.
const MARKED_CRATES: [(&str, fn() -> String); 4] = [
    ("crates/libs/rns-transport/src/", owner_tag),
    ("crates/apps/reticulumd/src/", app_crate_marker),
    ("crates/libs/meshchat-api/src/", app_crate_marker),
    ("crates/libs/lxmf-sdk/src/", app_crate_marker),
];

/// Tracked Rust sources under `crates/`, excluding this probe itself.
fn tracked_rust_sources() -> Vec<String> {
    git_tracked_files()
        .into_iter()
        .filter(|path| path.starts_with("crates/") && path.ends_with(".rs"))
        .filter(|path| !path.ends_with("lint_discipline_probe.rs"))
        .collect()
}

/// Whether a repo-relative path is test or example code by its location.
fn is_test_or_example_path(relative: &str) -> bool {
    let file = relative.rsplit('/').next().unwrap_or(relative);
    relative.contains("/tests/")
        || relative.contains("/tests_parts/")
        || relative.contains("/tests_sections/")
        || relative.contains("/examples/")
        || file == "tests.rs"
        || file.ends_with("_tests.rs")
}

/// The line span of the `mod <name> {` block whose body starts at `inner`
/// (the index of a line inside it), by brace depth from the `mod` line.
fn enclosing_mod_span(lines: &[&str], inner: usize) -> Option<(usize, usize)> {
    let start = lines[..inner].iter().rposition(|l| {
        let t = l.trim_start();
        (t.starts_with("mod ") || t.starts_with("pub mod ") || t.starts_with("pub(crate) mod "))
            && t.ends_with('{')
    })?;
    let mut depth = 0_i32;
    for (offset, line) in lines[start..].iter().enumerate() {
        for ch in line.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 0 {
            return Some((start, start + offset));
        }
    }
    None
}

/// No manifest in the repo mentions the lint: the workspace table does not
/// set it and no package table allows it, so it runs at its default level in
/// every crate.
#[test]
fn no_manifest_mentions_the_guard_lint() {
    let manifests: Vec<String> = git_tracked_files()
        .into_iter()
        .filter(|path| path == "Cargo.toml" || path.ends_with("/Cargo.toml"))
        .filter(|path| read_repo_file(path).contains(GUARD_LINT))
        .collect();
    assert!(
        manifests.is_empty(),
        "no Cargo.toml may mention {GUARD_LINT}; the lint runs at its default level and \
         allowances live in code, found {manifests:?}"
    );
}

/// Every allowance in production code of the four marked crates is fn-level,
/// sits directly on a fn, and names its owner on the same line: the exact
/// restructure marker in the app crates, the owner tag plus a site-specific
/// reason in rns-transport. The by-design handler-guard sites under
/// `src/transport/` are still there in number, and the app-crate sites are
/// exactly the measured ones.
#[test]
fn every_production_allow_of_the_guard_lint_is_fn_level_and_names_its_owner() {
    let needle = fn_level_allow();
    let handler_guard_marker = transport_marker();
    let mut handler_guard_sites = 0_usize;
    let mut app_sites: Vec<(String, usize)> = Vec::new();
    for path in tracked_rust_sources() {
        let Some((_, marker)) = MARKED_CRATES.iter().find(|(prefix, _)| path.starts_with(prefix))
        else {
            continue;
        };
        if is_test_or_example_path(&path) {
            continue;
        }
        let marker = marker();
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        let mut sites = 0_usize;
        for (index, line) in lines.iter().enumerate() {
            if !line.contains(&needle) {
                continue;
            }
            sites += 1;
            assert!(
                line.contains(&marker),
                "{path}:{}: a production allowance of {GUARD_LINT} carries `{marker}` on the same line",
                index + 1
            );
            if line.contains(&handler_guard_marker) {
                handler_guard_sites += 1;
            }
            let item = lines[index + 1..]
                .iter()
                .find(|l| !l.trim_start().starts_with("#["))
                .copied()
                .unwrap_or_default();
            assert!(
                item.contains("fn "),
                "{path}:{}: the allowance sits on a fn, not on `{}`",
                index + 1,
                item.trim()
            );
        }
        if sites > 0 && !path.starts_with("crates/libs/rns-transport/src/") {
            app_sites.push((path, sites));
        }
    }
    assert!(
        handler_guard_sites >= 30,
        "expected the by-design handler-guard sites under src/transport/, found {handler_guard_sites}"
    );
    app_sites.sort();
    let expected: Vec<(String, usize)> = [
        ("crates/apps/reticulumd/src/bin/reticulumd/announce_ingest.rs", 1),
        ("crates/apps/reticulumd/src/bin/reticulumd/bridge_propagation.rs", 1),
        ("crates/libs/lxmf-sdk/src/backend/zmq_pipeline/transport.rs", 2),
        ("crates/libs/lxmf-sdk/src/client/async_ops.rs", 1),
        ("crates/libs/meshchat-api/src/chat.rs", 1),
        ("crates/libs/meshchat-api/src/control.rs", 5),
        ("crates/libs/meshchat-api/src/lib.rs", 2),
    ]
    .into_iter()
    .map(|(path, sites)| (path.to_string(), sites))
    .collect();
    assert_eq!(
        app_sites, expected,
        "fn-level allowances outside src/transport/ sit on exactly the measured sites"
    );
}

/// Test roots allow the lint once, directly under the comment that says why,
/// and only inside a test root: no crate root takes it, and no test code
/// reaches for a per-site allowance.
#[test]
fn test_roots_allow_the_guard_lint_once_under_the_purpose_comment() {
    let module_needle = module_level_allow();
    let fn_needle = fn_level_allow();
    let mut roots = 0_usize;
    for path in tracked_rust_sources() {
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        let module_allows: Vec<usize> = lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.contains(&module_needle))
            .map(|(i, _)| i)
            .collect();
        assert!(
            module_allows.len() <= 1,
            "{path}: a root allows {GUARD_LINT} at most once, found {}",
            module_allows.len()
        );
        if is_test_or_example_path(&path) {
            assert!(
                !lines.iter().any(|l| l.contains(&fn_needle)),
                "{path}: test code takes the allowance at its root, never per site"
            );
        }
        let Some(&at) = module_allows.first() else {
            continue;
        };
        roots += 1;
        let previous =
            lines[..at].iter().rev().find(|l| !l.trim().is_empty()).copied().unwrap_or_default();
        let previous = previous.trim();
        assert!(
            previous.starts_with("// ") && previous.contains(TEST_ROOT_REASON),
            "{path}:{}: a test-root allowance sits directly under a comment saying it `{TEST_ROOT_REASON}`, got `{previous}`",
            at + 1,
        );
        // A `#[cfg(test)] mod tests { ... }` embedded in a production file:
        // the allowance must be inside that block (never at the crate root)
        // and nothing inside it may take the allowance again per site.
        let span = if is_test_or_example_path(&path) {
            (at, lines.len())
        } else {
            enclosing_mod_span(&lines, at).unwrap_or_else(|| panic!("{path}:{}: a production crate root allows {GUARD_LINT}; allowances outside test roots are fn-level", at + 1))
        };
        if let Some(offset) = lines[span.0..span.1].iter().position(|l| l.contains(&fn_needle)) {
            panic!(
                "{path}:{}: test code takes the allowance at its root, never per site",
                span.0 + offset + 1
            );
        }
    }
    assert!(roots >= 15, "expected the test roots to carry the allowance, found {roots}");
}

/// The lock discipline the markers point at is documented once, on
/// `TransportHandler`, and names the lock order the allowances rely on.
#[test]
fn transport_handler_rustdoc_states_the_lock_discipline_once() {
    let text = read_repo_file("crates/libs/rns-transport/src/transport/mod.rs");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("pub(crate) struct TransportHandler"))
        .expect("TransportHandler definition in transport/mod.rs");
    let doc: Vec<&str> =
        lines[..at].iter().rev().take_while(|l| l.starts_with("///")).copied().collect();
    // One line of prose, so a needle may span a wrapped line.
    let doc = doc
        .into_iter()
        .rev()
        .map(|l| l.trim_start_matches("///").trim())
        .collect::<Vec<_>>()
        .join(" ");
    for needle in [
        "The guard spans",
        "lock order",
        "handler, then `packet_cache`",
        "handler, then `iface_manager`",
        "handler, then a `Link`",
        "handler, then a destination",
        GUARD_LINT,
    ] {
        assert!(doc.contains(needle), "TransportHandler rustdoc must say `{needle}`, got:\n{doc}");
    }
    let documented_in: Vec<String> = tracked_rust_sources()
        .into_iter()
        .filter(|p| p.starts_with("crates/libs/rns-transport/src/"))
        .filter(|p| {
            read_repo_file(p).contains("fixes the\n/// lock order")
                || read_repo_file(p).contains("fixes the lock order")
        })
        .collect();
    assert_eq!(
        documented_in,
        vec!["crates/libs/rns-transport/src/transport/mod.rs".to_string()],
        "the lock discipline is documented once"
    );
}

/// The CI-equivalent classify script says the lint is live, refuses any
/// manifest that mentions it, and fails when a diagnostic fires outside the
/// per-target allowances; the Unreleased CHANGELOG bullet that announces the
/// lint-table change names the script.
#[test]
fn classify_script_and_changelog_say_the_guard_lint_is_live() {
    let script = read_repo_file("tools/scripts/workspace-clippy-classify.sh");
    for needle in [
        "await_holding_invalid_type is live at its\n# default level",
        "per fn with a same-line comment naming the owner",
        "git ls-files -z -- 'Cargo.toml' '*/Cargo.toml'",
        "FAIL: a Cargo.toml mentions await_holding_invalid_type",
        "fired outside the per-target allows (a production site or a new test root)",
        "zero await_holding_invalid_type diagnostics with the lint live at its default level",
    ] {
        assert!(script.contains(needle), "classify script must say `{needle}`");
    }
    for stale in ["despite the lint tables", "crate-level allows"] {
        assert!(!script.contains(stale), "the wording `{stale}` described the allow-table era");
    }
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let start = changelog.find("\n## Unreleased\n").expect("CHANGELOG Unreleased section") + 1;
    let rest = &changelog[start..];
    let unreleased = rest.find("\n## ").map_or(rest, |end| &rest[..end]);
    assert!(
        unreleased.contains("`clippy::await_holding_invalid_type`\n  runs at its default level"),
        "CHANGELOG Unreleased announces the lint running at its default level"
    );
    assert!(
        unreleased.contains("tools/scripts/workspace-clippy-classify.sh"),
        "CHANGELOG Unreleased names the classify script"
    );
}
