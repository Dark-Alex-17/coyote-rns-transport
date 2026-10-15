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
    let workspace_manifest = read_repo_file("Cargo.toml");

    assert_eq!(
        crate_version(),
        "0.14.1",
        "coyote-rns-transport should declare the published version; bump this pin with the crate"
    );

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
        "version = \"0.14.1\"",
        "description = \"Coyote's published fork of reticulum-rs-transport",
        "license.workspace = true",
        "rns-core.workspace = true",
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

    let core_dependency =
        "rns-core = { package = \"reticulum-rs-core\", version = \"0.14.0\", path = \"crates/libs/rns-core\" }";
    assert!(
        workspace_manifest.contains(core_dependency),
        "workspace manifest should pin the core dependency the published crate resolves through: {core_dependency:?}"
    );
}

/// Every site that names the published crate carries the same version, and
/// both release ledgers have a section for it. Cargo cannot catch a stale
/// dependent manifest on its own: a path dependency declared as
/// `version = "0.14.0"` is a caret requirement that 0.14.1 satisfies, so the
/// manifests, the consumer snippets and the lock file are pinned here.
#[test]
fn release_sites_and_ledgers_track_the_crate_version() {
    let version = crate_version();
    let version_token = format!("version = \"{version}\"");

    for manifest in
        ["Cargo.toml", "crates/apps/rns-tools/Cargo.toml", "crates/apps/reticulumd/Cargo.toml"]
    {
        let text = read_repo_file(manifest);
        let sites: Vec<&str> = text
            .lines()
            .filter(|line| line.contains("package = \"coyote-rns-transport\""))
            .collect();
        assert!(
            !sites.is_empty(),
            "{manifest} should depend on coyote-rns-transport by package name"
        );
        for site in sites {
            assert!(
                site.contains(&version_token),
                "{manifest} should require {version_token:?} on its coyote-rns-transport dependency, got {site:?}"
            );
        }
    }

    let consumer_line = format!("coyote-rns-transport = \"{version}\"");
    assert!(
        read_repo_file("docs/getting-started.md").contains(&consumer_line),
        "docs/getting-started.md should pin the consumer line {consumer_line:?}"
    );

    let lock_entry = format!("name = \"coyote-rns-transport\"\n{version_token}\n");
    assert!(
        read_repo_file("Cargo.lock").contains(&lock_entry),
        "Cargo.lock should carry the published crate at {version:?}; regenerate it with `cargo update -p coyote-rns-transport --offline`"
    );

    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let release_heading = format!("\n## {version} - ");
    assert!(
        changelog.contains(&release_heading),
        "crate CHANGELOG.md should have a dated `## {version} - YYYY-MM-DD` release heading"
    );
    let heading_offset = changelog.find(&release_heading).expect("release heading located");
    let dated = &changelog[heading_offset + release_heading.len()..];
    let date: String = dated.chars().take_while(|c| !c.is_whitespace()).collect();
    assert!(
        date.len() == 10
            && date.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }),
        "release heading date should be ISO YYYY-MM-DD, got {date:?}"
    );

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    for required in ["follows cargo semver", "docs/fork/PATCHSET.md", "CHANGELOG.md"] {
        assert!(
            readme.contains(required),
            "crate README should state the versioning policy and name both ledgers; missing {required:?}"
        );
    }
}

/// The patch-set ledger's section for the current release opens with a table
/// of every file changed since the previous tag. Every path in it must exist,
/// and the files that change on every release must be listed.
#[test]
fn patch_set_ledger_lists_existing_files_for_the_current_release() {
    let version = crate_version();
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section_heading = format!("\n## {version} (upstream ");
    let start = ledger.find(&section_heading).unwrap_or_else(|| {
        panic!("docs/fork/PATCHSET.md should have a `## {version} (upstream <base>)` section")
    });
    let section = &ledger[start + 1..];
    let section = match section[section_heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..section_heading.len() - 1 + end],
        None => section,
    };

    let root = repo_root();
    let table_paths: Vec<&str> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .filter_map(|line| {
            let rest = line.strip_prefix("| `")?;
            rest.split('`').next()
        })
        .collect();
    assert!(
        table_paths.len() >= 10,
        "the {version} section should open with a file table, found {} rows",
        table_paths.len()
    );

    let missing: Vec<&str> =
        table_paths.iter().copied().filter(|path| !root.join(path).exists()).collect();
    assert!(
        missing.is_empty(),
        "every file in the {version} table should exist in the tree:\n{}",
        missing.join("\n")
    );

    for required in [
        "Cargo.lock",
        "crates/libs/rns-transport/Cargo.toml",
        "crates/libs/rns-transport/CHANGELOG.md",
        "crates/apps/reticulumd/tests/fork_publish_contract.rs",
        "docs/fork/PATCHSET.md",
    ] {
        assert!(
            table_paths.contains(&required),
            "the {version} table should list {required:?}, which changes on every release"
        );
    }
}

/// Root `VERSION` is the workspace number: the upstream baseline that
/// `lxmf --version` prints and that HIL evidence records. The fork publishes
/// only `coyote-rns-transport`, so a fork patch release must not move it; it
/// stays equal to the unpublished workspace crates until the next upstream
/// rebase.
#[test]
fn root_version_stays_on_the_upstream_baseline() {
    let root_version = read_repo_file("VERSION");
    let mut tokens = root_version.split_whitespace();
    let root_version = tokens.next().expect("VERSION should contain a version token");
    assert!(tokens.next().is_none(), "VERSION should contain exactly one token");

    fn manifest_version(manifest: &str) -> String {
        read_repo_file(manifest)
            .lines()
            .find_map(|line| line.strip_prefix("version = \""))
            .and_then(|rest| rest.strip_suffix('"'))
            .unwrap_or_else(|| panic!("{manifest} should declare a package version"))
            .to_string()
    }

    for baseline in ["crates/apps/lxmf-cli/Cargo.toml", "crates/libs/rns-core/Cargo.toml"] {
        assert_eq!(
            manifest_version(baseline),
            root_version,
            "{baseline} is an upstream-baseline crate and should carry the root VERSION; \
             only coyote-rns-transport moves on a fork release"
        );
    }

    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    assert!(
        readme.contains("root `VERSION`"),
        "crate README should explain why root VERSION does not follow the crate version"
    );
}

/// Every row of the patch-set table names the patch it belongs to, and every
/// `B<n>` id used in a row is defined by the legend above the table.
#[test]
fn patch_set_ledger_rows_name_a_patch_from_the_legend() {
    let version = crate_version();
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section_heading = format!("\n## {version} (upstream ");
    let start = ledger
        .find(&section_heading)
        .unwrap_or_else(|| panic!("docs/fork/PATCHSET.md should have a `## {version}` section"));
    let section = &ledger[start + 1..];
    let section = match section[section_heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..section_heading.len() - 1 + end],
        None => section,
    };

    let table_start = section.find("\n| File | Patch |").unwrap_or_else(|| {
        panic!("the {version} section should open with a `| File | Patch |` table")
    });
    // The legend wraps at 80 columns, so phrases are matched on one line.
    let legend = section[..table_start].split_whitespace().collect::<Vec<_>>().join(" ");
    let legend = legend.as_str();
    let legend_ids: Vec<String> = (1..=5).map(|n| format!("B{n} is ")).collect();
    for id in &legend_ids {
        assert!(
            legend.contains(id.as_str()),
            "the {version} legend should define {:?} before the table",
            id.trim_end()
        );
    }

    let rows: Vec<(&str, &str)> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 2, "table row should have a file and a patch cell: {line:?}");
            (cells[0], cells[1])
        })
        .collect();
    assert!(!rows.is_empty(), "the {version} table should have rows");

    for (file, patch) in &rows {
        assert!(!patch.is_empty(), "{file} row should name the patch it belongs to");
        for phrase in patch.split(", ") {
            assert!(
                legend.contains(phrase),
                "{file} row names patch {phrase:?} which the legend does not define"
            );
            for id in phrase
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|token| token.len() == 2 && token.starts_with('B'))
            {
                assert!(
                    legend.contains(&format!("{id} is ")),
                    "{file} row uses patch id {id} which the legend does not define"
                );
            }
        }
    }

    let interop_row = rows
        .iter()
        .find(|(file, _)| file.contains("multi_segment_python_interop.rs"))
        .unwrap_or_else(|| {
            panic!("the {version} table should list the multi-segment interop test")
        });
    assert!(
        interop_row.1.contains("Python-in-the-loop"),
        "the multi-segment interop row should be attributed to the Python-in-the-loop proof, got {:?}",
        interop_row.1
    );
}

/// The README rebase recipe runs the same `cargo publish` the workflow runs.
/// Neither carries a `--registry` override: the publish workflow runs on a
/// GitHub runner with the default crates.io source, and a developer whose
/// local cargo config replaces that source adds the flag on the command line
/// rather than in the repository.
#[test]
fn publish_recipe_mirrors_the_workflow_without_a_registry_override() {
    let workflow = read_repo_file(".github/workflows/publish-coyote-rns-transport.yml");
    let readme = read_repo_file("crates/libs/rns-transport/README.md");

    let dry_run = "cargo publish -p coyote-rns-transport --locked --dry-run";
    assert!(
        workflow.contains(dry_run),
        "fork publish workflow should carry the dry-run publish command {dry_run:?}"
    );
    let recipe_step = readme
        .lines()
        .find(|line| line.contains("cargo publish"))
        .expect("crate README rebase recipe should carry a cargo publish step");
    assert!(
        recipe_step.contains(dry_run),
        "README recipe should run the workflow's dry-run publish command {dry_run:?}, got {recipe_step:?}"
    );

    for (name, text) in [("publish workflow", workflow.as_str()), ("crate README", readme.as_str())]
    {
        assert!(
            !text.contains("--registry"),
            "{name} should not carry a --registry override; local source replacement is a \
             per-machine workaround, not a repository contract"
        );
    }
}

/// Release-note bullets under the current `## <version>` heading of the crate
/// CHANGELOG end in a test clause that names the file pinning the behaviour.
/// Every backticked `.rs` / `.py` path cited in that section must resolve to
/// a file in the crate tree or the reticulumd interop tests, so a renamed or
/// deleted test file cannot leave the release notes pointing at nothing.
#[test]
fn release_notes_cite_files_that_exist_in_the_tree() {
    let version = crate_version();
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let heading = format!("\n## {version} - ");
    let start = changelog.find(&heading).unwrap_or_else(|| {
        panic!("crate CHANGELOG should have a dated `## {version} - <date>` heading")
    });
    let section = &changelog[start + 1..];
    let section = match section[heading.len() - 1..].find("\n## ") {
        Some(end) => &section[..heading.len() - 1 + end],
        None => section,
    };

    let mut cited: Vec<&str> = Vec::new();
    let mut rest = section;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        let is_path = (token.ends_with(".rs") || token.ends_with(".py"))
            && token.chars().all(|c| c.is_ascii_alphanumeric() || "/_.-".contains(c));
        if is_path && !cited.contains(&token) {
            cited.push(token);
        }
        rest = &after[close + 1..];
    }
    assert!(
        cited.len() >= 5,
        "the {version} release notes should carry per-bullet test clauses, found {} file cites",
        cited.len()
    );

    let root = repo_root();
    let mut tree: Vec<String> = Vec::new();
    for subtree in ["crates/libs/rns-transport", "crates/apps/reticulumd/tests"] {
        visit_text_files(&root, &root.join(subtree), &mut |path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            tree.push(relative.to_string_lossy().replace('\\', "/"));
        });
    }

    let unresolved: Vec<&str> = cited
        .iter()
        .copied()
        .filter(|token| !tree.iter().any(|path| path.ends_with(&format!("/{token}"))))
        .collect();
    assert!(
        unresolved.is_empty(),
        "every file cited in the {version} release notes should exist under the crate or the \
         reticulumd tests:\n{}",
        unresolved.join("\n")
    );
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

/// Genuinely historical subtrees under docs/: goal records, migration notes,
/// decision records, and per-release notes describe the tree as it was when
/// they were written, so the old package name is correct there. Everything
/// else under docs/ (including docs/status/, the current roadmap and parity
/// records, and docs/sdk/) is scanned so runnable commands stay current.
const SKIPPED_DOC_PREFIXES: [&str; 4] =
    ["docs/goals/", "docs/migrations/", "docs/adr/", "docs/release-notes-"];

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
