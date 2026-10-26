// Spec-first pins for the python-remote-relay CI fix (the thirteenth and
// fourteenth patches of 0.16.0): the release-notes cite gate resolves files
// under `crates/apps/lxmf-cli/tests` without accepting paths that do not
// exist there; the 0.16.0 CHANGELOG bullet that restructures the two
// Python-in-the-loop tests cites the files that now carry them, and every
// cite resolves; the two-peer relay test lives only in its new part and
// the CI workflow still names it; the ledger legend and header carry both
// commits. Each test fails against the tree as it was before the fix.

/// The `## 0.16.0` section of the crate changelog, raw.
fn changelog_0_16_0_section() -> String {
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let (_, rest) = changelog
        .split_once("\n## 0.16.0 - ")
        .expect("the CHANGELOG has a released 0.16.0 section");
    rest.split("\n## ").next().unwrap_or(rest).to_string()
}

/// The lxmf-cli test tree is a cite root: a real file there resolves both as
/// a repo-relative path and as a path suffix, and a file that does not exist
/// under that root is still reported, so widening the roots did not loosen
/// the gate.
#[test]
fn cite_gate_resolves_lxmf_cli_test_files_and_still_rejects_missing_ones() {
    let real = [
        "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/shared_instance_two_peer_relay.rs",
        "crates/apps/lxmf-cli/tests/support/python_lxmd_remote_relay_parts/status_hash.rs",
        "crates/apps/lxmf-cli/tests/support/python_lxmf_endpoint.py",
        "python_lxmd_remote_relay_parts/python_router_restart.rs",
    ];
    for path in real {
        assert!(
            repo_root().join(path).exists() || path.starts_with("python_lxmd_remote_relay_parts/"),
            "probe premise: {path} exists in the tree"
        );
    }
    let unresolved = unresolved_source_cites(&real);
    assert!(unresolved.is_empty(), "every lxmf-cli test cite should resolve, got {unresolved:?}");

    let missing = [
        "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/shared_instance_three_peer_relay.rs",
        "python_lxmd_remote_relay_parts/no_such_part.rs",
        "crates/apps/lxmf-cli/src/main.rs",
    ];
    assert!(
        !repo_root().join(missing[0]).exists(),
        "probe premise: the bogus part name is not in the tree"
    );
    let unresolved = unresolved_source_cites(&missing);
    assert_eq!(
        unresolved,
        missing.to_vec(),
        "a cite that names no file under the lxmf-cli test root (or sits outside the roots) is \
         still unresolved"
    );
}

/// The 0.16.0 bullet that restructures the two Python-in-the-loop tests is
/// marked test-only, names both test functions, cites the new part, the
/// router-restart part, the Python endpoint and the widened cite gate, and
/// every file it cites exists.
#[test]
fn changelog_0_16_0_remote_relay_bullet_cites_the_files_that_carry_the_tests() {
    let section = changelog_0_16_0_section();
    let bullet = section
        .split("\n- ")
        .find(|bullet| {
            bullet
                .contains("python_shared_instance_two_peer_relay_recovers_after_daemon_restart_e2e")
        })
        .expect("the 0.16.0 notes carry a bullet for the two-peer relay test restructure");
    assert!(
        bullet.starts_with("No library behaviour change (CI):"),
        "the restructure bullet is a test-only Changed bullet, got {:?}",
        bullet.lines().next().unwrap_or_default()
    );
    assert!(
        bullet.contains("python_lxmf_router_delivery_cache_survives_process_restart_e2e"),
        "the bullet names the router-restart test as well"
    );
    let cited = cited_source_files(bullet);
    for expected in [
        "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/shared_instance_two_peer_relay.rs",
        "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/python_router_restart.rs",
        "crates/apps/lxmf-cli/tests/support/python_lxmf_endpoint.py",
        "crates/apps/reticulumd/tests/fork_publish_contract.rs",
    ] {
        assert!(cited.contains(&expected), "the bullet cites {expected}, got {cited:?}");
    }
    let unresolved = unresolved_source_cites(&cited);
    assert!(unresolved.is_empty(), "every cite in the bullet resolves, got {unresolved:?}");
    assert!(
        bullet.contains("`local_client_count`"),
        "the bullet names the new Python status key the router-restart test waits on"
    );
}

/// The two-peer relay test is defined exactly once, in its new part, which
/// the test root includes, and the CI workflow still selects it by name.
#[test]
fn two_peer_relay_test_lives_only_in_its_new_part_and_ci_still_names_it() {
    let name = "python_shared_instance_two_peer_relay_recovers_after_daemon_restart_e2e";
    let parts = "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts";
    let new_part = read_repo_file(&format!("{parts}/shared_instance_two_peer_relay.rs"));
    let old_part = read_repo_file(&format!("{parts}/shared_instance_daemon.rs"));
    assert_eq!(
        new_part.matches(&format!("fn {name}()")).count(),
        1,
        "the new part defines the test once"
    );
    assert!(!old_part.contains(name), "shared_instance_daemon.rs no longer carries the test");
    for part in [&new_part, &old_part] {
        assert!(
            part.lines().count() <= 1200,
            "both parts stay within the 1200-line test-file budget"
        );
    }
    let root = read_repo_file("crates/apps/lxmf-cli/tests/python_lxmd_remote_relay.rs");
    assert!(
        root.contains(
            "include!(\"python_lxmd_remote_relay_parts/shared_instance_two_peer_relay.rs\");"
        ),
        "the test root includes the new part"
    );
    let workflow = read_repo_file(".github/workflows/verify.yml");
    assert!(workflow.contains(name), "verify.yml still names the test");
    // One shared instance per daemon: each relay config carries a single
    // local_client_interface entry, and the uplink is a Rust TCP pair.
    assert_eq!(
        new_part.matches("local_client_interface(").count(),
        2,
        "two relays, one attach entry each"
    );
    assert!(new_part.contains("tcp_client_interface(\"relay-a-uplink\""), "the relays are bridged");
    assert!(
        !new_part.contains("require_attached_local_clients("),
        "no relay is asked to carry more than one attached shared instance"
    );
}

/// The router-restart test waits on the live Python observable before the
/// single post-restart announce, and keeps the static Rust-side check.
#[test]
fn router_restart_test_waits_for_the_live_reattach_before_the_announce() {
    let part = read_repo_file(
        "crates/apps/lxmf-cli/tests/python_lxmd_remote_relay_parts/python_router_restart.rs",
    );
    let require = part
        .rfind("require_attached_local_client(rust_rpc_port, \"Rust client after Python restart\")")
        .expect("the static Rust-side attach check stays");
    let wait = part
        .find("wait_for_python_local_client_count(python_control_port, 1,")
        .expect("the test waits for the Python shared instance to report the re-attached client");
    let announce = part[wait..]
        .find("python_control_call(python_control_port, \"announce\", None)")
        .map(|offset| wait + offset)
        .expect("a Python announce follows the wait");
    assert!(require < wait && wait < announce, "order: static check, live wait, announce");
    assert!(
        part.contains("write_python_shared_instance_rns_config_with_control_port("),
        "the Python RNS config reserves an instance_control_port"
    );
    let helper = read_repo_file(
        "crates/apps/lxmf-cli/tests/support/python_lxmd_remote_relay_parts/status_hash.rs",
    );
    assert!(
        helper.contains("Duration::from_secs(60)")
            && helper.contains("[\"reticulum\"][\"local_client_count\"]"),
        "the helper polls reticulum.local_client_count under a 60 s bound"
    );
    let endpoint = read_repo_file("crates/apps/lxmf-cli/tests/support/python_lxmf_endpoint.py");
    assert!(
        endpoint.contains("\"local_client_count\": len(")
            && endpoint.contains("\"local_client_interfaces\""),
        "the Python endpoint derives local_client_count from RNS.Transport.local_client_interfaces"
    );
}

/// The 0.16.0 ledger carries both fix commits in its legend and header and
/// the Python shared-instance config writer has no port-less variant left.
#[test]
fn ledger_0_16_0_carries_both_remote_relay_fix_commits() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let section = ledger_section(&ledger, "0.16.0").expect("the 0.16.0 ledger section");
    let legend = ledger_legend("0.16.0", section);
    let header = ledger_header_hashes(section);
    for hash in ["7b168319", "00c419a0"] {
        assert!(legend.contains(&format!("`{hash}`")), "the legend names {hash}");
        assert!(header.contains(&hash), "the header names {hash}, got {header:?}");
    }
    let prelude = read_repo_file(
        "crates/apps/lxmf-cli/tests/support/python_lxmd_remote_relay_parts/module_prelude.rs",
    );
    assert!(
        !prelude.contains("fn write_python_shared_instance_rns_config(")
            && !prelude.contains("write_python_shared_instance_rns_config_inner"),
        "the port-less shared-instance config writer is gone"
    );
    assert!(
        prelude.contains("instance_control_port = {control_port}"),
        "the remaining writer always reserves the instance control port"
    );
}
