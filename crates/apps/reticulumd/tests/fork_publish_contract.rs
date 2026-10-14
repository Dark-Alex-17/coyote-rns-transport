use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn crate_version() -> String {
    read_repo_file("crates/libs/rns-transport/Cargo.toml")
        .lines()
        .find_map(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("coyote-rns-transport manifest should declare a package version")
        .to_string()
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
        "workflow_dispatch:",
        "push:",
        "tags:",
        "- 'coyote-rns-transport-v*'",
        "      ref:\n        description: Git ref, tag, or SHA to publish from",
        "dry_run",
        "type: boolean",
        "libdbus-1-dev",
        "dtolnay/rust-toolchain@stable",
        "CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}",
        "cargo publish -p coyote-rns-transport --locked",
        "--dry-run",
        "PUBLISH_REF: ${{ inputs.ref || github.ref_name }}",
        "run: tools/scripts/check-publish-tag-version.sh",
        "concurrency:",
        "default: main",
    ] {
        assert!(
            workflow.contains(required),
            "fork publish workflow should include required token {required:?}"
        );
    }
}

#[test]
fn publish_tag_version_guard_pins_its_contract() {
    let script = read_repo_file("tools/scripts/check-publish-tag-version.sh");

    for required in [
        "cargo pkgid --locked -p coyote-rns-transport",
        "[ \"$crate_version\" != \"$tag_version\" ]",
        "exit 1",
    ] {
        assert!(
            script.contains(required),
            "publish tag version guard should include required token {required:?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn publish_tag_version_guard_rejects_a_mismatched_tag() {
    use std::process::Command;

    let script = repo_root().join("tools/scripts/check-publish-tag-version.sh");
    let run = |publish_ref: Option<&str>| {
        let mut command = Command::new("bash");
        command.arg(&script).current_dir(repo_root());
        match publish_ref {
            Some(publish_ref) => command.env("PUBLISH_REF", publish_ref),
            None => command.env_remove("PUBLISH_REF"),
        };
        let output = command.output().expect("run check-publish-tag-version.sh");
        (output.status.code(), String::from_utf8_lossy(&output.stdout).into_owned())
    };

    let (code, stdout) = run(Some("coyote-rns-transport-v9.9.9"));
    assert_ne!(code, Some(0), "a mismatched release tag must fail the guard");
    assert!(
        stdout.contains("::error::Tag coyote-rns-transport-v9.9.9 names version 9.9.9"),
        "mismatch output should name the tag version, got {stdout:?}"
    );

    let matching_tag = format!("coyote-rns-transport-v{}", crate_version());
    let (code, stdout) = run(Some(&matching_tag));
    assert_eq!(code, Some(0), "tag {matching_tag:?} should pass the guard, got {stdout:?}");

    let (code, stdout) = run(Some("main"));
    assert_eq!(code, Some(0), "a non-tag ref should pass the guard, got {stdout:?}");
    assert!(
        stdout.contains("is not a coyote-rns-transport-v* release tag"),
        "non-tag output should say the check was skipped, got {stdout:?}"
    );

    let (code, _) = run(None);
    assert_eq!(code, Some(2), "an unset PUBLISH_REF should exit with the usage code");
}

#[test]
fn crate_readme_pins_the_consumer_contract() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let manifest = read_repo_file("crates/libs/rns-transport/Cargo.toml");

    let consumer_line = format!("coyote-rns-transport = \"{}\"", crate_version());
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
        "description = \"Coyote's published fork of reticulum-rs-transport",
        "license.workspace = true",
        "[lib]",
        "name = \"rns_transport\"",
        "readme = \"README.md\"",
        "repository = \"https://github.com/Dark-Alex-17/coyote-rns-transport\"",
        "documentation = \"https://docs.rs/coyote-rns-transport\"",
        "keywords = [\"reticulum\", \"transport\", \"network\", \"links\"]",
        "categories = [\"network-programming\"]",
    ] {
        assert!(
            manifest.contains(required),
            "coyote-rns-transport manifest should include required token {required:?}"
        );
    }
}

#[test]
fn repo_readme_points_at_the_crate_readme() {
    let pointer = "](crates/libs/rns-transport/README.md)";
    assert!(
        read_repo_file("README.md").contains(pointer),
        "repository README should link to the crate README with {pointer:?}"
    );
    assert!(
        repo_root().join("crates/libs/rns-transport/README.md").is_file(),
        "the crate README the repository README points at should exist"
    );
}

/// The lxmd smoke script waits for the warn's own message, `close /<32 hex>/`,
/// in the Rust lxmd log, so the needle is independent of how the tracing
/// subscriber renders the level and target. Re-applying the patch must keep
/// the message starting with `close ` followed by the link id.
#[test]
fn link_close_needle_is_derivable_from_the_surviving_warn() {
    let needle = "close /[0-9a-f]{32}/";

    let smoke = read_repo_file("tools/scripts/python-lxmd-rust-lxmd-smoke.sh");
    let waits = smoke.matches(&format!("\"{needle}\"")).count();
    assert!(
        waits >= 2,
        "lxmd smoke script should wait for the {needle:?} needle at both teardown sites, found {waits}"
    );

    let source = read_repo_file(
        "crates/libs/rns-transport/src/destination/link_parts/link_sections/next_watchdog_deadline.rs",
    );
    let warn_call = "log::warn!(\"close {}\", self.id)";
    assert!(source.contains(warn_call), "link close must keep the warn {warn_call:?}");
    assert!(!source.contains("println!"), "link close must not print to stdout");

    let literal = &warn_call[warn_call.find('"').expect("warn call opens a literal") + 1..];
    let prefix = &literal[..literal.find("{}").expect("warn literal interpolates the id")];
    assert!(
        needle.starts_with(prefix),
        "needle {needle:?} should start with the warn message prefix {prefix:?}"
    );
    assert_eq!(
        &needle[prefix.len()..],
        "/[0-9a-f]{32}/",
        "the rest of the needle should match the link id's Display: a slash, 32 lowercase hex digits, a slash"
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

/// Historical subtrees under docs/ (goals, migrations, decision records,
/// status reports, SDK notes, release notes) describe the tree as it was when
/// they were written; the old package name is correct there. Everything else
/// under docs/ is scanned so runnable commands stay current.
const SKIPPED_DOC_PREFIXES: [&str; 6] = [
    "docs/goals/",
    "docs/migrations/",
    "docs/adr/",
    "docs/status/",
    "docs/sdk/",
    "docs/release-notes-",
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
