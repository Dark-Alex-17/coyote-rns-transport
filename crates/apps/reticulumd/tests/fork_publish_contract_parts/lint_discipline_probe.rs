// Usage pins for the guard discipline: the `clippy::await_holding_invalid_type`
// lint runs at its default level under the workspace clippy invocation, no
// lint table mentions it, every production allowance is fn-level and names
// its owner on the same line, test roots allow it once under a comment that
// says why, the fns that deny it keep doing so, and the lock discipline the
// transport markers point at is documented on `TransportHandler`.
//
// The needles are assembled with `concat!` and `format!` so this file never
// contains the attribute or marker text it scans for.

/// The lint name as it appears in lint tables and attributes.
const GUARD_LINT: &str = "await_holding_invalid_type";

/// The lint path as an attribute names it.
fn lint_path() -> String {
    format!("clippy::{GUARD_LINT}")
}

/// A module-level allowance: `#![allow(clippy::<lint>)]`.
fn module_level_allow() -> String {
    format!("#![allow(clippy::{GUARD_LINT})]")
}

/// Whether a line is an outer attribute that relaxes the lint in any
/// spelling: the plain fn-level `allow`, a composite list with the lint
/// anywhere in it, an `expect`, or either nested in `cfg_attr`. Inner
/// (`#![...]`) attributes are roots, not sites, and a `deny` or `forbid`
/// tightens rather than relaxes.
fn is_site_allowance(line: &str) -> bool {
    let attribute = line.trim_start();
    attribute.starts_with("#[")
        && attribute.contains(&lint_path())
        && !attribute.contains("deny(")
        && !attribute.contains("forbid(")
}

/// Whether a line is an inner attribute naming the lint in a spelling other
/// than the single sanctioned test-root allowance or a plain `deny`.
fn is_unsanctioned_root_attribute(line: &str) -> bool {
    let attribute = line.trim_start();
    attribute.starts_with("#![")
        && attribute.contains(&lint_path())
        && attribute != module_level_allow()
        && !attribute.contains("deny(")
}

/// The fn-level denial: `#[deny(clippy::<lint>)]`.
fn fn_level_deny() -> String {
    format!("#[deny(clippy::{GUARD_LINT})]")
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

/// A source-path prefix and the builder of the marker text its allowances carry.
type MarkedCrate = (&'static str, fn() -> String);

/// Production sources whose allowances this probe pins, with the marker text
/// each must carry on the same line. Production code anywhere else allows
/// the lint nowhere.
const MARKED_CRATES: [MarkedCrate; 4] = [
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

/// The reticulumd manifest neither declares a `[lints]` table nor inherits
/// the workspace one: inheriting would newly enable `unwrap_used` on the
/// crate, and a package table is where the allow-table era kept the lint.
#[test]
fn reticulumd_manifest_has_no_lint_table_and_does_not_inherit_the_workspace_one() {
    let manifest = read_repo_file("crates/apps/reticulumd/Cargo.toml");
    let tables: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter(|line| line.starts_with("[lints") || line.contains("lints.workspace"))
        .collect();
    assert!(tables.is_empty(), "reticulumd/Cargo.toml must not carry a lints table, found {tables:?}");
    assert!(
        manifest.contains("# No [lints] workspace = true on purpose"),
        "reticulumd/Cargo.toml says why it stays out of workspace lint inheritance"
    );
}

/// Every allowance in production code sits in one of the four marked crates,
/// is fn-level, sits directly on a fn, and names its owner on the same line:
/// the exact restructure marker in the app crates, the owner tag plus a
/// site-specific reason in rns-transport. The rns-transport sites are still
/// there in number, as are the by-design handler-guard sites among them, and
/// the app-crate sites are exactly the measured ones.
#[test]
fn every_production_allow_of_the_guard_lint_is_fn_level_and_names_its_owner() {
    let handler_guard_marker = transport_marker();
    let mut transport_sites = 0_usize;
    let mut handler_guard_sites = 0_usize;
    let mut app_sites: Vec<(String, usize)> = Vec::new();
    let mut unmarked_sites: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) {
            continue;
        }
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        let Some((_, marker)) = MARKED_CRATES.iter().find(|(prefix, _)| path.starts_with(prefix))
        else {
            unmarked_sites.extend(
                lines
                    .iter()
                    .enumerate()
                    .filter(|(_, line)| is_site_allowance(line))
                    .map(|(index, _)| format!("{path}:{}", index + 1)),
            );
            continue;
        };
        let marker = marker();
        let mut sites = 0_usize;
        for (index, line) in lines.iter().enumerate() {
            if !is_site_allowance(line) {
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
        if path.starts_with("crates/libs/rns-transport/src/") {
            transport_sites += sites;
        } else if sites > 0 {
            app_sites.push((path, sites));
        }
    }
    assert!(
        unmarked_sites.is_empty(),
        "production code outside the marked crates allows {GUARD_LINT} nowhere, found {unmarked_sites:?}"
    );
    assert!(
        transport_sites >= 35,
        "expected the owner-tagged rns-transport sites, found {transport_sites}"
    );
    assert!(
        handler_guard_sites >= 25,
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
/// in that one spelling, and only inside a test root: no crate root takes
/// it, no root relaxes it through a composite or `expect` attribute, and no
/// test code reaches for a per-site allowance.
#[test]
fn test_roots_allow_the_guard_lint_once_under_the_purpose_comment() {
    let module_needle = module_level_allow();
    let mut roots = 0_usize;
    for path in tracked_rust_sources() {
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        if let Some(at) = lines.iter().position(|l| is_unsanctioned_root_attribute(l)) {
            panic!(
                "{path}:{}: a root relaxes {GUARD_LINT} only through the single `{module_needle}`",
                at + 1
            );
        }
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
                !lines.iter().any(|l| is_site_allowance(l)),
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
        if let Some(offset) = lines[span.0..span.1].iter().position(|l| is_site_allowance(l)) {
            panic!(
                "{path}:{}: test code takes the allowance at its root, never per site",
                span.0 + offset + 1
            );
        }
    }
    assert!(roots >= 15, "expected the test roots to carry the allowance, found {roots}");
}

/// The fns that deny the lint outright keep doing so: the denial is the
/// positive half of the discipline, so the count stays in the hundreds and
/// the named carriers (the announce entry points, the request dispatcher,
/// the resource link lookup) each still carry it.
#[test]
fn fn_level_deny_of_the_guard_lint_stays_on_its_carriers() {
    let needle = fn_level_deny();
    let mut denials = 0_usize;
    let mut per_file: Vec<(String, usize)> = Vec::new();
    for path in tracked_rust_sources() {
        let sites = read_repo_file(&path).lines().filter(|line| line.contains(&needle)).count();
        denials += sites;
        per_file.push((path, sites));
    }
    assert!(denials >= 100, "expected the fn-level denials of {GUARD_LINT} to stay, found {denials}");
    for (carrier, item) in [
        ("crates/libs/rns-transport/src/transport/core.rs", "pub async fn try_send_announce("),
        ("crates/libs/rns-transport/src/transport/request_dispatch.rs", "pub(super) async fn run("),
        ("crates/libs/rns-transport/src/transport/resource_wire.rs", "pub(super) async fn link_by_id("),
    ] {
        let sites = per_file.iter().find(|(path, _)| path == carrier).map_or(0, |(_, n)| *n);
        assert!(sites > 0, "{carrier} keeps a fn-level denial of {GUARD_LINT}");
        let text = read_repo_file(carrier);
        let lines: Vec<&str> = text.lines().collect();
        let at = lines
            .iter()
            .position(|line| line.trim_start().starts_with(item))
            .unwrap_or_else(|| panic!("{carrier} defines `{item}`"));
        let attributes: Vec<&str> =
            lines[..at].iter().rev().take_while(|l| l.trim_start().starts_with("#[")).copied().collect();
        assert!(
            attributes.iter().any(|line| line.contains(&needle)),
            "{carrier}: `{item}` carries the fn-level denial of {GUARD_LINT}, got {attributes:?}"
        );
    }
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
/// manifest that mentions it before it spends a clippy run, and fails when a
/// diagnostic fires outside the per-target allowances; the CHANGELOG bullet
/// that announces the lint-table change names the script. The CHANGELOG is
/// read whole with whitespace normalised: the bullet wraps, and the section
/// it sits under is renamed at release.
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
    let gate = script.find("FAIL: a Cargo.toml mentions").expect("the manifest gate");
    let clippy_run = script.find("cargo clippy --workspace").expect("the clippy run");
    assert!(gate < clippy_run, "the manifest gate runs before the clippy run");
    let ls_files = script.find("git ls-files -z -- 'Cargo.toml'").expect("the manifest listing");
    let listing_line = script[ls_files..].lines().next().unwrap_or_default();
    assert!(
        !listing_line.contains("|| true") && !listing_line.contains('|'),
        "the manifest listing fails the script on its own; only the grep may tolerate no match, got `{listing_line}`"
    );
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        changelog.contains("`clippy::await_holding_invalid_type` runs at its default level"),
        "CHANGELOG announces the lint running at its default level"
    );
    assert!(
        changelog.contains("tools/scripts/workspace-clippy-classify.sh"),
        "CHANGELOG names the classify script"
    );
}

/// A staged manifest that allows the lint fails the classify script at the
/// manifest gate, by name, before any cargo run: the clone's reticulumd
/// manifest gets a `[lints.clippy]` table with the lint allowed, the
/// working-tree script is run over the clone, and the refusal arrives in
/// well under the time a clippy run would take.
#[cfg(unix)]
#[test]
fn classify_script_refuses_a_staged_manifest_that_allows_the_guard_lint() {
    let script = "workspace-clippy-classify.sh";
    let manifest = "crates/apps/reticulumd/Cargo.toml";
    let clone = ProbeClone::new("classify-manifest-gate");
    fs::copy(repo_root().join("tools/scripts").join(script), clone.path("tools/scripts").join(script))
        .expect("copy the working-tree classify script into the clone");
    let mut staged = fs::read_to_string(clone.path(manifest)).expect("read the clone's manifest");
    staged.push_str(&format!("\n[lints.clippy]\n{GUARD_LINT} = \"allow\"\n"));
    fs::write(clone.path(manifest), staged).expect("write the clone's manifest");
    clone.git(&["add", manifest]);

    let started = std::time::Instant::now();
    let (code, stdout, stderr) = clone.run_script(script, &[]);
    assert_eq!(code, Some(1), "a manifest allowing {GUARD_LINT} fails the gate: {stderr:?}");
    assert!(
        stdout.contains("FAIL: a Cargo.toml mentions await_holding_invalid_type"),
        "the refusal names the gate, got stdout {stdout:?} stderr {stderr:?}"
    );
    assert!(stdout.contains(manifest), "the refusal names the manifest, got {stdout:?}");
    assert!(
        !stdout.contains("cargo clippy (CI invocation) exit"),
        "the gate refuses before the clippy run, got {stdout:?}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "the refusal must not wait on a clippy run"
    );
}
