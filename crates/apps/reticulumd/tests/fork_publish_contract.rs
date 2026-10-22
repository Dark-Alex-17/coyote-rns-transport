use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The lockfiles a release regenerates: the workspace lock and the fuzz crate's
/// own lock, the fuzz crate not being a workspace member.
const RELEASE_LOCKS: [&str; 2] = ["Cargo.lock", "crates/libs/rns-transport/fuzz/Cargo.lock"];

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

/// Patch ids in the ledger legend are one uppercase letter and one or two
/// digits, such as `B3`, `F1` or `F10`.
fn is_patch_id(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(letter) = chars.next() else { return false };
    let digits = chars.as_str();
    letter.is_ascii_uppercase()
        && (1..=2).contains(&digits.len())
        && digits.bytes().all(|byte| byte.is_ascii_digit())
}

#[test]
fn is_patch_id_accepts_one_letter_and_one_or_two_digits() {
    for accepted in ["F1", "F10", "F99", "H2"] {
        assert!(is_patch_id(accepted), "{accepted:?} is a patch id");
    }
    for rejected in ["F", "F100", "f1", "1F", "F1x", "FX", "10"] {
        assert!(!is_patch_id(rejected), "{rejected:?} is not a patch id");
    }
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
        "PUBLISH_REF=\"${PUBLISH_REF#refs/tags/}\"",
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
    assert_eq!(code, Some(1), "a mismatched release tag must fail the guard with exit 1");
    assert!(
        stdout.contains("::error::Tag coyote-rns-transport-v9.9.9 names version 9.9.9"),
        "mismatch output should name the tag version, got {stdout:?}"
    );

    let matching_tag = format!("coyote-rns-transport-v{}", crate_version());
    let (code, stdout) = run(Some(&matching_tag));
    assert_eq!(code, Some(0), "tag {matching_tag:?} should pass the guard, got {stdout:?}");

    let full_ref = format!("refs/tags/{matching_tag}");
    let (code, stdout) = run(Some(&full_ref));
    assert_eq!(code, Some(0), "full ref {full_ref:?} should pass the guard, got {stdout:?}");
    assert!(
        !stdout.contains("is not a coyote-rns-transport-v* release tag"),
        "full ref {full_ref:?} should be checked as a release tag, not skipped, got {stdout:?}"
    );

    let (code, stdout) = run(Some("refs/tags/coyote-rns-transport-v9.9.9"));
    assert_eq!(
        code,
        Some(1),
        "a mismatched release tag given as a full ref must fail the guard with exit 1"
    );
    assert!(
        stdout.contains("::error::Tag coyote-rns-transport-v9.9.9 names version 9.9.9"),
        "full-ref mismatch output should name the tag version, got {stdout:?}"
    );

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
        "0.15.1",
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
        "version = \"0.15.1\"",
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
/// both release ledgers have a section for it. On a patch release Cargo
/// cannot catch a stale dependent manifest on its own: a path dependency
/// declared as `version = "0.14.0"` is a caret requirement that 0.14.1
/// satisfies, so the manifests, the consumer snippets and the lock file are
/// pinned here.
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
    // The fuzz crate has a lock of its own (it is not a workspace member), and
    // nothing but this pin notices when a release leaves it behind.
    for lock in RELEASE_LOCKS {
        assert!(
            read_repo_file(lock).contains(&lock_entry),
            "{lock} should carry the published crate at {version:?}; regenerate it with \
             `cargo update -p coyote-rns-transport --offline` (add `--manifest-path \
             crates/libs/rns-transport/fuzz/Cargo.toml` for the fuzz lock)"
        );
    }

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

/// The fuzz crate is not a workspace member, so its `Cargo.lock` is the one
/// release site the workspace `cargo update` never touches. The README's
/// release recipe has to name every lock the version pin above checks, with
/// the `--manifest-path` the fuzz lock needs, or the next release leaves it
/// behind exactly the way 0.14.1 did.
#[test]
fn release_recipe_regenerates_every_lock_the_version_pin_checks() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let regenerate = readme
        .find("cargo update -p coyote-rns-transport --offline")
        .expect("crate README should carry the lock-regeneration step of the release recipe");
    // The numbered recipe step that carries the command: from its `N. ` opener
    // to the next step's opener (or the end of the file).
    let opens_a_step = |line: &str| {
        let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
        digits > 0 && line[digits..].starts_with(". ")
    };
    let step_start = readme[..regenerate]
        .rmatch_indices('\n')
        .map(|(at, _)| at + 1)
        .find(|&at| opens_a_step(&readme[at..]))
        .unwrap_or(0);
    let step_end = readme[regenerate..]
        .match_indices('\n')
        .map(|(at, _)| regenerate + at + 1)
        .find(|&at| opens_a_step(&readme[at..]))
        .unwrap_or(readme.len());
    let recipe = &readme[step_start..step_end];

    for lock in RELEASE_LOCKS {
        assert!(
            recipe.contains(&format!("`{lock}`")),
            "the README release recipe should name {lock:?} among the locks to regenerate:\n{recipe}"
        );
    }
    assert!(
        recipe.contains("--manifest-path crates/libs/rns-transport/fuzz/Cargo.toml"),
        "the README release recipe should give the `--manifest-path` the fuzz lock needs:\n{recipe}"
    );
}

/// The fuzz crate depends on the library by path, so its lock must resolve
/// `coyote-rns-transport` to exactly one path package at the crate version:
/// a second, registry-sourced entry would mean the fuzz targets exercise the
/// published crate instead of the tree they sit in.
#[test]
fn fuzz_lock_resolves_the_crate_from_its_path_dependency() {
    let manifest = read_repo_file("crates/libs/rns-transport/fuzz/Cargo.toml");
    let path_dependency = manifest
        .lines()
        .find(|line| line.contains("package = \"coyote-rns-transport\""))
        .expect("fuzz Cargo.toml should depend on coyote-rns-transport by package name");
    assert!(
        path_dependency.contains("path = \"..\""),
        "fuzz Cargo.toml should take coyote-rns-transport from the parent directory, got {path_dependency:?}"
    );

    let lock = read_repo_file("crates/libs/rns-transport/fuzz/Cargo.lock");
    let packages: Vec<&str> = lock.split("[[package]]").skip(1).collect();
    let crate_entries: Vec<&str> = packages
        .iter()
        .copied()
        .filter(|entry| entry.contains("name = \"coyote-rns-transport\"\n"))
        .collect();
    assert_eq!(
        crate_entries.len(),
        1,
        "fuzz Cargo.lock should resolve coyote-rns-transport exactly once, got {}",
        crate_entries.len()
    );
    let entry = crate_entries[0];
    assert!(
        !entry.contains("source = "),
        "fuzz Cargo.lock should take coyote-rns-transport from the path dependency, not a registry:\n{entry}"
    );
    let version_token = format!("version = \"{}\"\n", crate_version());
    assert!(
        entry.contains(&version_token),
        "fuzz Cargo.lock should carry coyote-rns-transport at {version_token:?}:\n{entry}"
    );

    let fuzz_entry = packages
        .iter()
        .copied()
        .find(|entry| entry.contains("name = \"rns-transport-fuzz\"\n"))
        .expect("fuzz Cargo.lock should carry the fuzz crate itself");
    let depends_on_crate = fuzz_entry
        .split("dependencies = [")
        .nth(1)
        .is_some_and(|deps| deps.lines().any(|line| line.trim() == "\"coyote-rns-transport\","));
    assert!(
        depends_on_crate,
        "the fuzz crate's lock entry should list coyote-rns-transport as a dependency:\n{fuzz_entry}"
    );
}

/// The `## <label> (upstream <base>)` section of the patch-set ledger, up to
/// the next `## ` heading. `label` is a release version or `Unreleased`.
/// The anchor ends in a space so `0.15.1` cannot match a `0.15.10` heading.
/// A `## <label> ` heading that does not continue with `(upstream <base>)`
/// is a malformed ledger, not a missing section, so it panics rather than
/// disabling the checks that key on the heading.
fn ledger_section<'a>(ledger: &'a str, label: &str) -> Option<&'a str> {
    let heading_anchor = format!("\n## {label} ");
    let start = ledger.find(&heading_anchor)?;
    let section = &ledger[start + 1..];
    let heading = section.lines().next().unwrap_or_default();
    assert!(
        heading[heading_anchor.len() - 1..].starts_with("(upstream "),
        "the ledger heading for {label} should read `## {label} (upstream <base>)`, got {heading:?}"
    );
    Some(match section[heading.len()..].find("\n## ") {
        Some(end) => &section[..heading.len() + end],
        None => section,
    })
}

/// The ledger section of the current crate version, which every release
/// must have.
fn release_ledger_section(ledger: &str) -> (String, &str) {
    let version = crate_version();
    let section = ledger_section(ledger, &version).unwrap_or_else(|| {
        panic!("docs/fork/PATCHSET.md should have a `## {version} (upstream <base>)` section")
    });
    (version, section)
}

/// The legend prose above a section's `| File | Patch |` table, joined onto
/// one line because the ledger wraps at 80 columns and phrases are matched
/// whole.
fn ledger_legend(label: &str, section: &str) -> String {
    let table_start = section.find("\n| File | Patch |").unwrap_or_else(|| {
        panic!("the {label} section should open with a `| File | Patch |` table")
    });
    section[..table_start].split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The patch-set ledger's section for the current release opens with a table
/// of every file changed since the previous tag. Every path in it must exist,
/// and the files that change on every release must be listed.
#[test]
fn patch_set_ledger_lists_existing_files_for_the_current_release() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let (version, section) = release_ledger_section(&ledger);
    check_ledger_section_lists_existing_files(&version, section, true);
}

/// The `## Unreleased` section, when present, is held to the same file table
/// contract short of the rows only a release adds: one row is enough and the
/// files that change on every release need not be there yet.
#[test]
fn patch_set_ledger_unreleased_section_lists_existing_files() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let Some(section) = ledger_section(&ledger, "Unreleased") else { return };
    check_ledger_section_lists_existing_files("Unreleased", section, false);
}

fn check_ledger_section_lists_existing_files(
    label: &str,
    section: &str,
    require_release_rows: bool,
) {
    let root = repo_root();
    let table_paths: Vec<&str> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .filter_map(|line| {
            let rest = line.strip_prefix("| `")?;
            rest.split('`').next()
        })
        .collect();
    let floor = if require_release_rows { 10 } else { 1 };
    assert!(
        table_paths.len() >= floor,
        "the {label} section should open with a file table, found {} rows",
        table_paths.len()
    );

    let missing: Vec<&str> =
        table_paths.iter().copied().filter(|path| !root.join(path).exists()).collect();
    assert!(
        missing.is_empty(),
        "every file in the {label} table should exist in the tree:\n{}",
        missing.join("\n")
    );

    if !require_release_rows {
        return;
    }

    let required_every_release = [
        "Cargo.toml",
        "crates/apps/rns-tools/Cargo.toml",
        "crates/apps/reticulumd/Cargo.toml",
        "crates/libs/rns-transport/Cargo.toml",
        "crates/libs/rns-transport/CHANGELOG.md",
        "crates/libs/rns-transport/README.md",
        "docs/getting-started.md",
        "crates/apps/reticulumd/tests/fork_publish_contract.rs",
        "docs/fork/PATCHSET.md",
    ];
    for required in RELEASE_LOCKS.iter().chain(required_every_release.iter()) {
        assert!(
            table_paths.contains(required),
            "the {label} table should list {required:?}, which changes on every release"
        );
    }
}

/// Root `VERSION` is the workspace number: the upstream baseline that
/// `lxmf --version` prints and that HIL evidence records. The fork publishes
/// only `coyote-rns-transport`, so a fork release must not move it; it
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
/// letter-digit patch id used in a row is defined by the legend above the table.
#[test]
fn patch_set_ledger_rows_name_a_patch_from_the_legend() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let (version, section) = release_ledger_section(&ledger);
    check_ledger_rows_name_a_patch_from_the_legend(&version, section, true);
}

/// Rows of the `## Unreleased` table, when present, name legend patches the
/// same way; the Python-in-the-loop proof is owed by a release, not by work
/// in progress.
#[test]
fn patch_set_ledger_unreleased_rows_name_a_patch_from_the_legend() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let Some(section) = ledger_section(&ledger, "Unreleased") else { return };
    check_ledger_rows_name_a_patch_from_the_legend("Unreleased", section, false);
}

fn check_ledger_rows_name_a_patch_from_the_legend(
    label: &str,
    section: &str,
    require_python_proof: bool,
) {
    let legend = ledger_legend(label, section);
    let legend = legend.as_str();
    let defines_a_patch_id = legend
        .split(' ')
        .any(|token| is_patch_id(token) && legend.contains(&format!("{token} is ")));
    assert!(
        defines_a_patch_id,
        "the {label} legend should define at least one patch id before the table"
    );

    let rows: Vec<(&str, &str)> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
            assert_eq!(cells.len(), 2, "table row should have a file and a patch cell: {line:?}");
            (cells[0], cells[1])
        })
        .collect();
    assert!(!rows.is_empty(), "the {label} table should have rows");

    for (file, patch) in &rows {
        assert!(!patch.is_empty(), "{file} row should name the patch it belongs to");
        for phrase in patch.split(", ") {
            assert!(
                legend.contains(phrase),
                "{file} row names patch {phrase:?} which the legend does not define"
            );
            for id in phrase
                .split(|c: char| !c.is_ascii_alphanumeric())
                .filter(|token| is_patch_id(token))
            {
                assert!(
                    legend.contains(&format!("{id} is ")),
                    "{file} row uses patch id {id} which the legend does not define"
                );
            }
        }
    }

    if !require_python_proof {
        return;
    }

    // Every release carries a Python-in-the-loop proof: at least one interop
    // fixture row is attributed to a patch named for it.
    let interop_rows: Vec<&(&str, &str)> =
        rows.iter().filter(|(file, _)| file.contains("/python_channel_interop_parts/")).collect();
    let found = if interop_rows.is_empty() {
        "the table has no interop rows".to_string()
    } else {
        let rows: Vec<String> =
            interop_rows.iter().map(|(file, patch)| format!("{file} ({patch})")).collect();
        format!("interop rows found: {}", rows.join("; "))
    };
    assert!(
        interop_rows.iter().any(|(_, patch)| patch.contains("Python-in-the-loop")),
        "the {label} table should attribute at least one `python_channel_interop_parts/` row to a \
         patch whose legend phrase contains `Python-in-the-loop`: every release section owes a \
         Python-in-the-loop proof, and the ledger shows it by naming the patch for it; {found}"
    );
}

/// A backticked eight-digit short hash, with any trailing sentence
/// punctuation.
fn commit_hash(token: &str) -> Option<&str> {
    let hash = token.trim_end_matches([',', '.', ';', ':']).strip_prefix('`')?.strip_suffix('`')?;
    let is_hash =
        hash.len() == 8 && hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
    is_hash.then_some(hash)
}

/// The hashes named by the section's `Fork commit(s) ...:` patch headers.
/// Headers wrap: the hashes run from the `Fork commit` line through the
/// first colon, which may sit on a later line. The legend above the table
/// is skipped so a legend sentence that happens to open with `Fork commits`
/// cannot vouch for itself.
fn ledger_header_hashes(section: &str) -> Vec<&str> {
    let table_start = section
        .find("\n| File | Patch |")
        .expect("the section should open with a `| File | Patch |` table");
    let mut header_hashes = Vec::new();
    let mut in_header = false;
    for line in section[table_start..].lines() {
        if line.starts_with("Fork commit `") || line.starts_with("Fork commits `") {
            in_header = true;
        }
        if !in_header {
            continue;
        }
        let hashes = line.split_once(':').map_or(line, |(before, _)| before);
        header_hashes.extend(hashes.split(' ').filter_map(commit_hash));
        in_header = !line.contains(':');
    }
    header_hashes
}

/// The legend hashes no patch header of the section names.
fn legend_hashes_without_a_header<'a>(section: &str, legend_hashes: &[&'a str]) -> Vec<&'a str> {
    let header_hashes = ledger_header_hashes(section);
    legend_hashes.iter().copied().filter(|hash| !header_hashes.contains(hash)).collect()
}

/// The legend names the fork commits of the release patch; every one of them
/// is a commit a rebase must re-apply, so each must also appear in a patch
/// header of the section. Hashes added to the legend alone are the drift this
/// catches.
#[test]
fn patch_set_ledger_release_legend_hashes_each_have_a_patch_header() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let (version, section) = release_ledger_section(&ledger);
    let legend = ledger_legend(&version, section);
    let release_patch = format!("`release {version}` patch");
    let release_at = legend.find(&release_patch).unwrap_or_else(|| {
        panic!("the {version} legend should name the `release {version}` patch")
    });
    let commits = &legend[release_at + release_patch.len()..];
    let commits_at = commits.find("fork commits").unwrap_or_else(|| {
        panic!("the {version} legend should list the fork commits of the release patch")
    });
    let commits = &commits[commits_at + "fork commits".len()..];

    let mut legend_hashes = Vec::new();
    for token in commits.split(' ') {
        legend_hashes.extend(commit_hash(token));
        if token.ends_with('.') {
            break;
        }
    }
    assert!(
        !legend_hashes.is_empty(),
        "the {version} legend should name at least one fork commit for the release patch"
    );

    let missing = legend_hashes_without_a_header(section, &legend_hashes);
    assert!(
        missing.is_empty(),
        "every fork commit the {version} legend names for the release patch should head a patch \
         in the section, since each patch header lists every commit a rebase must re-apply; \
         missing: {}",
        missing.join(", ")
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

/// The distinct backticked `.rs` / `.py` paths in `section`, in order of
/// first appearance.
fn cited_source_files(section: &str) -> Vec<&str> {
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
    cited
}

/// The cites that name no file under the crate tree, the reticulumd interop
/// tests or the rns-tools sources, either as a repo-relative path or as a
/// path suffix.
fn unresolved_source_cites<'a>(cited: &[&'a str]) -> Vec<&'a str> {
    let root = repo_root();
    let mut tree: Vec<String> = Vec::new();
    for subtree in
        ["crates/libs/rns-transport", "crates/apps/reticulumd/tests", "crates/apps/rns-tools/src"]
    {
        visit_text_files(&root, &root.join(subtree), &mut |path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            tree.push(relative.to_string_lossy().replace('\\', "/"));
        });
    }
    cited
        .iter()
        .copied()
        .filter(|token| {
            !tree.iter().any(|path| path == token || path.ends_with(&format!("/{token}")))
        })
        .collect()
}

/// Release-note bullets under the current `## <version>` heading of the crate
/// CHANGELOG end in a test clause that names the file pinning the behaviour.
/// Every backticked `.rs` / `.py` path cited in that section must resolve to
/// a file in the crate tree, the reticulumd interop tests or the rns-tools
/// sources, so a renamed or deleted test file cannot leave the release notes
/// pointing at nothing.
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

    let cited = cited_source_files(section);
    assert!(
        cited.len() >= 5,
        "the {version} release notes should carry per-bullet test clauses, found {} file cites",
        cited.len()
    );

    let unresolved = unresolved_source_cites(&cited);
    assert!(
        unresolved.is_empty(),
        "every file cited in the {version} release notes should exist under the crate, the \
         reticulumd tests or the rns-tools sources:\n{}",
        unresolved.join("\n")
    );
}

/// Bullets under `## Unreleased` cite their pinning files the same way the
/// released sections do, and are the ones most likely to drift while a test
/// file is still being renamed. When the section is present its heading
/// carries no date, every top-level bullet cites at least one pinning file,
/// and every cite must resolve under the same roots; a CHANGELOG without the
/// section has nothing to check.
#[test]
fn unreleased_notes_cite_files_that_exist_in_the_tree() {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let Some(start) = changelog.find("\n## Unreleased") else { return };
    let (heading, body) = changelog[start + 1..].split_once('\n').unwrap_or_else(|| {
        panic!("the `## Unreleased` heading should be followed by release notes")
    });
    assert_eq!(
        heading, "## Unreleased",
        "the Unreleased heading takes no date or suffix until it is released, got {heading:?}"
    );
    let section = match body.find("\n## ") {
        Some(end) => &body[..end],
        None => body,
    };

    let uncited = changelog_bullets_without_a_cite("Unreleased", section);
    assert!(
        uncited.is_empty(),
        "every Unreleased bullet should cite at least one file that pins it; these cite none:\n{}",
        uncited.join("\n")
    );

    let unresolved = unresolved_source_cites(&cited_source_files(section));
    assert!(
        unresolved.is_empty(),
        "every file cited in the Unreleased release notes should exist under the crate, the \
         reticulumd tests or the rns-tools sources:\n{}",
        unresolved.join("\n")
    );
}

include!("fork_publish_contract_parts/ledger_contract.rs");

include!("fork_publish_contract_parts/ledger_contract_fixtures.rs");

include!("fork_publish_contract_parts/release_tooling_probe.rs");

include!("fork_publish_contract_parts/release_tooling_probe2.rs");

include!("fork_publish_contract_parts/release_tooling_probe3.rs");

include!("fork_publish_contract_parts/release_tooling_probe4.rs");

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
            // Dot-dirs like .coyote and .venv are local tool state; .github holds the publish workflow.
            let local_tool_dir = name.starts_with('.') && name != ".github";
            if !local_tool_dir && !SKIPPED_DIRECTORIES.contains(&name) {
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

/// Every file git would commit from this checkout: tracked files plus
/// untracked ones no ignore rule excludes, so local tool state (`.coyote/`,
/// `plans/`, anything in `.git/info/exclude`) is never scanned.
fn git_committable_files() -> Vec<String> {
    git_ls_files(&["--cached", "--others", "--exclude-standard"])
}

/// Every file git tracks in this checkout, so a file that only exists on
/// disk (left behind by a release run, not yet added) does not count.
fn git_tracked_files() -> Vec<String> {
    git_ls_files(&["--cached"])
}

fn git_ls_files(selection: &[&str]) -> Vec<String> {
    let output = Command::new("git")
        .arg("ls-files")
        .args(selection)
        .arg("-z")
        .current_dir(repo_root())
        .output()
        .expect("run git ls-files");
    assert!(
        output.status.success(),
        "git ls-files should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git ls-files paths should be UTF-8")
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_string)
        .collect()
}

/// The same filters `visit_text_files` applies, on a repo-relative path.
fn is_scanned_for_the_old_package_name(relative: &str) -> bool {
    let path = Path::new(relative);
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let in_skipped_dir = path.parent().into_iter().flat_map(Path::components).any(|dir| {
        let dir = dir.as_os_str().to_str().unwrap_or_default();
        SKIPPED_DIRECTORIES.contains(&dir) || (dir.starts_with('.') && dir != ".github")
    });
    let scanned = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| SCANNED_EXTENSIONS.contains(&ext));
    scanned
        && !in_skipped_dir
        && name != "Cargo.lock"
        && !SKIPPED_DOC_PREFIXES.iter().any(|prefix| relative.starts_with(prefix))
}

#[test]
fn old_package_name_survives_only_as_prose() {
    let root = repo_root();
    let mut offenders = Vec::new();
    // This file spells out the forbidden forms as needles.
    let this_file = "crates/apps/reticulumd/tests/fork_publish_contract.rs";
    for relative in git_committable_files() {
        if relative == this_file || !is_scanned_for_the_old_package_name(&relative) {
            continue;
        }
        let Ok(text) = fs::read_to_string(root.join(&relative)) else { continue };
        for (index, line) in text.lines().enumerate() {
            if OLD_PACKAGE_NAME_FORMS.iter().any(|form| line.contains(form)) {
                offenders.push(format!("{relative}:{}: {}", index + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the upstream package name must not survive as a cargo target or manifest name:\n{}",
        offenders.join("\n")
    );
}

/// Release hygiene the fork carries in CI: the publish contract runs on the
/// pull-request path, not only on the tag workflow.
#[test]
fn pull_request_ci_runs_the_publish_contract() {
    let ci = read_repo_file(".github/workflows/ci.yml");
    assert!(ci.contains("\non:\n  pull_request:"), "ci.yml should run on pull requests");
    assert!(
        ci.contains("run: cargo test -p reticulumd --test fork_publish_contract --test fork_release_docs_contract"),
        "ci.yml should run the fork publish and release-docs contracts as one step"
    );
    // The step belongs to a job with no `if:` guard, so it runs on every PR.
    let job_start = ci[..ci.find("--test fork_publish_contract").expect("step located")]
        .rfind("\n  contracts:\n")
        .expect("the publish contract step should sit in the `contracts` job");
    let job = &ci[job_start..ci.find("--test fork_publish_contract").expect("step located")];
    assert!(
        !job.lines().any(|line| line.trim_start().starts_with("if:")),
        "the `contracts` job must not be conditional, or the publish contract skips on PRs"
    );
}

// The job body: the `  contracts:` line and every following line that is blank or
// indented deeper than a top-level job key.
fn contracts_job(workflow: &str, name: &str) -> String {
    let start = workflow
        .find("\n  contracts:\n")
        .unwrap_or_else(|| panic!("{name} should have a `contracts` job"));
    workflow[start + 1..]
        .lines()
        .enumerate()
        .take_while(|(i, line)| *i == 0 || line.is_empty() || line.starts_with("    "))
        .map(|(_, line)| format!("{line}\n"))
        .collect()
}

/// Every `crates/libs/rns-transport/tests/*.rs` target runs on the
/// pull-request path, not only the compile-fail harness, and the fork
/// patch-set ledger is checked against the release tags, which the script
/// can only see on a full-history checkout.
#[test]
fn pull_request_ci_runs_every_external_rns_transport_test_target() {
    let ci = read_repo_file(".github/workflows/ci.yml");
    let job = contracts_job(&ci, "ci.yml");
    assert!(
        job.contains("run: cargo test -p coyote-rns-transport --tests"),
        "ci.yml's contracts job should run every external coyote-rns-transport test target"
    );
    assert!(
        job.contains("run: tools/scripts/patchset-ledger-check.sh"),
        "ci.yml's contracts job should run the fork patch-set ledger check"
    );
    let checkout = &job[..job.find("run:").expect("the contracts job should have a run step")];
    assert!(
        checkout.contains("fetch-depth: 0"),
        "the contracts job checkout needs the full history and tags for the ledger check, got {checkout:?}"
    );
}

/// The `contracts` job of `ci-full.yml` is the twin of the one in `ci.yml`:
/// every step the pull-request workflow runs (the docs build, the fork
/// publish and release-docs contracts, the breaking-change compile-fail
/// harness, the external test targets, the patch-set ledger check) runs
/// there too, on the same full-history checkout the ledger check needs, so
/// the dispatch workflow cannot pass on a tree the PR workflow would reject.
#[test]
fn ci_full_contracts_job_runs_every_contract_step_ci_runs() {
    let ci = read_repo_file(".github/workflows/ci.yml");
    let ci_full = read_repo_file(".github/workflows/ci-full.yml");
    let ci_job = contracts_job(&ci, "ci.yml");
    let ci_full_job = contracts_job(&ci_full, "ci-full.yml");
    let steps: Vec<&str> =
        ci_job.lines().filter(|line| line.trim_start().starts_with("run: ")).collect();
    assert!(
        steps.len() >= 5,
        "ci.yml's contracts job should run at least five steps, got {steps:?}"
    );
    for step in steps {
        assert!(ci_full_job.contains(step), "ci-full.yml's contracts job should carry {step:?}");
    }
    let checkout = &ci_full_job
        [..ci_full_job.find("run:").expect("ci-full.yml's contracts job should have a run step")];
    assert!(
        checkout.contains("fetch-depth: 0"),
        "ci-full.yml's contracts job checkout needs the full history and tags for the ledger \
         check, got {checkout:?}"
    );
}
