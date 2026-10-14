use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

#[test]
fn upstream_publish_workflow_is_disabled() {
    let workflow = read_repo_file(".github/workflows/crates-io-publish.yml");

    for required in [
        "if: false",
        "Disabled in this fork: publishing happens only through publish-coyote-rns-transport.yml.",
    ] {
        assert!(
            workflow.contains(required),
            "upstream crates.io publish workflow should include required token {required:?}"
        );
    }
}

#[test]
fn fork_publish_workflow_pins_its_contract() {
    let workflow = read_repo_file(".github/workflows/publish-coyote-rns-transport.yml");

    for required in [
        "coyote-rns-transport-v*",
        "dry_run",
        "type: boolean",
        "libdbus-1-dev",
        "dtolnay/rust-toolchain@stable",
        "CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}",
        "cargo publish -p coyote-rns-transport --locked",
        "--dry-run",
        "PUBLISH_REF: ${{ inputs.ref || github.ref_name }}",
        "cargo pkgid --locked -p coyote-rns-transport",
        "concurrency:",
        "default: main",
    ] {
        assert!(
            workflow.contains(required),
            "fork publish workflow should include required token {required:?}"
        );
    }

    assert!(
        !workflow.contains("environment:"),
        "fork publish workflow must not require a deployment environment"
    );
}

#[test]
fn crate_readme_pins_the_consumer_contract() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");

    let version = manifest
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("coyote-rns-transport manifest should declare a package version");
    let consumer_line = format!("coyote-rns-transport = \"{version}\"");
    assert!(
        readme.contains(&consumer_line),
        "coyote-rns-transport README should pin the consumer line {consumer_line:?}"
    );

    for required in [
        "coyote-rns-transport-v",
        "CARGO_REGISTRY_TOKEN",
        "EPL-2.0 OR GPL-2.0-or-later",
        "rns_transport",
    ] {
        assert!(
            readme.contains(required),
            "coyote-rns-transport README should include required token {required:?}"
        );
    }

    for required in [
        "name = \"coyote-rns-transport\"",
        "[lib]",
        "name = \"rns_transport\"",
        "readme = \"README.md\"",
        "repository = \"https://github.com/Dark-Alex-17/coyote-rns-transport\"",
        "documentation = \"https://docs.rs/coyote-rns-transport\"",
    ] {
        assert!(
            manifest.contains(required),
            "coyote-rns-transport manifest should include required token {required:?}"
        );
    }
}

/// The lxmd smoke script waits for `link: close` in the Rust lxmd log. Since
/// the stdout print was removed, that text is produced by the tracing
/// subscriber rendering the surviving `log::warn!` as `<target>: <message>`,
/// where the target is `rns_transport::destination::link`. That rendering
/// holds only with ANSI colour off: with it on, the target and the colon are
/// painted as separate escape-wrapped spans and the needle cannot match, so
/// the smoke launches lxmd with `NO_COLOR=1`. The target itself holds only
/// because next_watchdog_deadline.rs is pulled in through `include!` rather
/// than declared as a module, so this test pins the include chain: turning
/// the includes into `mod` declarations would move the target to the file's
/// own path and silently break the needle.
#[test]
fn link_close_needle_is_derivable_from_the_surviving_warn() {
    let needle = "link: close";

    let smoke = read_repo_file("tools/scripts/python-lxmd-rust-lxmd-smoke.sh");
    assert!(
        smoke.contains(&format!("\"{needle}\"")),
        "lxmd smoke script should wait for the {needle:?} needle"
    );
    let launch = smoke
        .lines()
        .find(|line| line.contains("target/debug/lxmd") && line.contains("RUST_LOG="))
        .expect("lxmd smoke script should launch the Rust lxmd daemon");
    assert!(
        launch.contains("NO_COLOR=1"),
        "lxmd launch line {launch:?} should set NO_COLOR=1 so the tracing target renders plain"
    );

    let src = "crates/libs/rns-transport/src";

    let mut target = vec!["rns_transport"];
    for (file, module) in [("lib.rs", "destination"), ("destination.rs", "link")] {
        let declaration = format!("pub mod {module};");
        assert!(
            read_repo_file(&format!("{src}/{file}")).contains(&declaration),
            "{file} should declare {declaration:?}"
        );
        target.push(module);
    }
    let target = target.join("::");

    for (file, included) in [
        ("destination/link.rs", "link_parts/link.rs"),
        ("destination/link_parts/link.rs", "link_sections/next_watchdog_deadline.rs"),
    ] {
        let directive = format!("include!(\"{included}\");");
        assert!(
            read_repo_file(&format!("{src}/{file}")).contains(&directive),
            "{file} should reach the link close warn through {directive:?}, not a mod declaration"
        );
    }

    let source = read_repo_file(&format!(
        "{src}/destination/link_parts/link_sections/next_watchdog_deadline.rs"
    ));
    let warn_call = "log::warn!(\"close {}\", self.id)";
    assert!(source.contains(warn_call), "link close must keep the warn {warn_call:?}");
    assert!(!source.contains("println!"), "link close must not print to stdout");

    let literal = &warn_call[warn_call.find('"').expect("warn call opens a literal") + 1..];
    let prefix = &literal[..literal.find("{}").expect("warn literal interpolates the id")];

    let rendered = format!("{target}: {prefix}");
    assert!(
        rendered.contains(needle),
        "rendered warn line {rendered:?} should contain the needle {needle:?}"
    );
}

const OLD_PACKAGE_NAME_FORMS: [&str; 9] = [
    "-p reticulum-rs-transport",
    "--package reticulum-rs-transport",
    "\"-p\", \"reticulum-rs-transport\"",
    "\"reticulum-rs-transport\"",
    "package = \"reticulum-rs-transport\"",
    "package: \"reticulum-rs-transport\"",
    "name = \"reticulum-rs-transport\"",
    "reticulum-rs-transport = \"",
    "crates.io/crates/reticulum-rs-transport",
];

const SKIPPED_DIRECTORIES: [&str; 5] = ["target", ".git", "vendor", "artifacts", "generated"];

/// Historical records under docs/ that describe the tree as it was when they
/// were written; the old package name is correct there. Live runbooks and
/// interface docs are scanned so their runnable commands stay current.
const SKIPPED_DOC_PREFIXES: [&str; 10] = [
    "docs/goals/",
    "docs/migrations/",
    "docs/adr/",
    "docs/status/",
    "docs/sdk/",
    "docs/release-notes-",
    "docs/runbooks/crates-io-publish-plan.md",
    "docs/runbooks/release-readiness.md",
    "docs/lxmf-rs-api.md",
    "docs/performance/tcp-connection-memory.md",
];

const SCANNED_EXTENSIONS: [&str; 8] = ["rs", "toml", "yml", "yaml", "sh", "py", "md", "txt"];

fn is_skipped_doc(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let relative = relative.to_string_lossy();
    SKIPPED_DOC_PREFIXES.iter().any(|prefix| relative.starts_with(prefix))
}

fn visit_text_files(root: &Path, dir: &Path, f: &mut impl FnMut(&Path)) {
    let entries =
        fs::read_dir(dir).unwrap_or_else(|err| panic!("read directory {}: {err}", dir.display()));
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if is_skipped_doc(root, &path) {
            continue;
        }
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name) {
                visit_text_files(root, &path, f);
            }
            continue;
        }
        if name == "Cargo.lock" {
            continue;
        }
        let scanned = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| SCANNED_EXTENSIONS.contains(&ext));
        if scanned && path.is_file() {
            f(&path);
        }
    }
}

#[test]
fn old_package_name_survives_only_as_prose() {
    let root = repo_root();
    let this_file = root.join("crates/apps/reticulumd/tests/fork_publish_contract.rs");
    let mut offenders = Vec::new();

    visit_text_files(&root, &root, &mut |path| {
        // This file spells out the forbidden forms as needles.
        if path == this_file {
            return;
        }
        let Ok(text) = fs::read_to_string(path) else {
            return;
        };
        for (index, line) in text.lines().enumerate() {
            if OLD_PACKAGE_NAME_FORMS.iter().any(|form| line.contains(form)) {
                let relative = path.strip_prefix(&root).unwrap_or(path);
                offenders.push(format!("{}:{}: {}", relative.display(), index + 1, line.trim()));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "the upstream package name must not survive as a cargo target or manifest name:\n{}",
        offenders.join("\n")
    );
}
