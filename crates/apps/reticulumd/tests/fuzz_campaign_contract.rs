//! The fuzz campaign script, the runbook that tells a release engineer to
//! run it, and the fuzz crates it drives must agree: every target the
//! script names is a `[[bin]]` of the crate directory it points at, the
//! script addresses crates the way `cargo fuzz` accepts (`--fuzz-dir`), and
//! the runbook lists every target the script runs.

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

fn read_repo_file(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// `(fuzz_dir, target)` for every `run_target <dir> <target>` line.
fn campaign_targets(script: &str) -> Vec<(String, String)> {
    script
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("run_target "))
        .map(|rest| {
            let mut parts = rest.split_whitespace();
            let dir = parts.next().expect("run_target has a fuzz dir").to_string();
            let target = parts.next().expect("run_target has a target").to_string();
            assert!(parts.next().is_none(), "run_target takes exactly two args: {rest}");
            (dir, target)
        })
        .collect()
}

fn bin_names(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("name = \""))
        .filter_map(|rest| rest.strip_suffix('"'))
        .map(str::to_string)
        .collect()
}

#[test]
fn campaign_script_addresses_fuzz_crates_with_fuzz_dir_not_manifest_path() {
    let script = read_repo_file("tools/scripts/fuzz-campaign.sh");

    assert!(
        !script.contains("--manifest-path"),
        "cargo-fuzz 0.13 has no --manifest-path option; the campaign must use --fuzz-dir"
    );
    assert!(script.contains("--fuzz-dir \"$fuzz_dir\""), "run_target passes --fuzz-dir");
    assert!(
        script.contains("cargo +\"$TOOLCHAIN\" fuzz run"),
        "every target runs on the configured (nightly by default) toolchain"
    );
    assert!(script.contains("TOOLCHAIN=\"${FUZZ_TOOLCHAIN:-nightly}\""), "FUZZ_TOOLCHAIN override");
    assert!(
        script.contains("find crates/libs/*/fuzz/artifacts -type f"),
        "the campaign fails when any target leaves a crash artifact"
    );
}

#[test]
fn every_campaign_target_is_a_bin_of_the_fuzz_crate_it_names() {
    let script = read_repo_file("tools/scripts/fuzz-campaign.sh");
    let targets = campaign_targets(&script);
    assert!(targets.len() >= 5, "expected rpc, sdk and three transport targets, got {targets:?}");

    for (dir, target) in &targets {
        let manifest_path = format!("{dir}/Cargo.toml");
        assert!(repo_root().join(dir).is_dir(), "campaign fuzz dir {dir} does not exist");
        let manifest = read_repo_file(&manifest_path);
        assert!(
            manifest.contains("cargo-fuzz = true"),
            "{manifest_path} is not a cargo-fuzz crate"
        );
        assert!(
            bin_names(&manifest).iter().any(|name| name == target),
            "{manifest_path} declares no [[bin]] named {target}"
        );
        let target_source = format!("{dir}/fuzz_targets/{target}.rs");
        assert!(
            repo_root().join(&target_source).is_file(),
            "campaign target source {target_source} does not exist"
        );
    }

    let transport: Vec<&str> = targets
        .iter()
        .filter(|(dir, _)| dir == "crates/libs/rns-transport/fuzz")
        .map(|(_, target)| target.as_str())
        .collect();
    assert_eq!(
        transport,
        ["request_frame_decode", "response_frame_decode", "resource_advertisement_decode"],
        "the transport crate runs a target for each of its decoders"
    );
}

#[test]
fn fuzzing_runbook_lists_every_target_the_campaign_runs() {
    let script = read_repo_file("tools/scripts/fuzz-campaign.sh");
    let runbook = read_repo_file("docs/runbooks/fuzzing-campaign.md");

    for (dir, target) in campaign_targets(&script) {
        let listed = format!("`{dir}/fuzz_targets/{target}.rs`");
        assert!(runbook.contains(&listed), "runbook does not list {listed}");
    }
    assert!(runbook.contains("FUZZ_MAX_TOTAL_TIME=300 ./tools/scripts/fuzz-campaign.sh"));
    assert!(runbook.contains("rustup toolchain install nightly"));
    assert!(runbook.contains("cargo run -p xtask -- sdk-fuzz-check"));
}
