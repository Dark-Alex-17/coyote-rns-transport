use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().expect("repo root")
}

#[test]
fn local_interface_smoke_preserves_software_evidence_contract() {
    let root = repo_root();
    let script_path = root.join("tools/scripts/local-interface-smoke.sh");
    let script = fs::read_to_string(&script_path).expect("read LocalInterface smoke script");

    for required in [
        "target/local-interface-smoke",
        "LocalInterface",
        "LocalClientInterface",
        "local-tcp-listener",
        "local-tcp-attach",
        "shared_instance_type = \"tcp\"",
        "host = \"127.0.0.1\"",
        "fixed_mtu = 262144",
        "force_shared_instance_bitrate = 1000000",
        "--strict-interface-startup",
        "rnstatus-rs",
        "accepted_connections",
        "startup_status",
        "active",
        "attached",
        "runtime_iface",
        "shared_instance_type",
        "force_shared_instance_bitrate",
        "report.json",
    ] {
        assert!(
            script.contains(required),
            "LocalInterface smoke should include required token {required:?}"
        );
    }
}

#[test]
fn local_interface_unix_smoke_preserves_software_evidence_contract() {
    let root = repo_root();
    let script_path = root.join("tools/scripts/local-interface-unix-smoke.sh");
    let script = fs::read_to_string(&script_path).expect("read LocalInterface Unix smoke script");

    for required in [
        "target/local-interface-unix-smoke",
        "LocalInterface",
        "LocalClientInterface",
        "local-unix-filesystem-listener",
        "local-unix-abstract-listener",
        "local-unix-abstract-attach",
        "shared_instance_type = \"unix\"",
        "socket_path = \"${FILESYSTEM_SOCKET}\"",
        "socket_path = \"@rns/${ABSTRACT_ATTACH_INSTANCE}\"",
        "instance_name = \"${ABSTRACT_LISTENER_INSTANCE}\"",
        "fixed_mtu = 262144",
        "force_shared_instance_bitrate = 1000000",
        "--strict-interface-startup",
        "rnstatus-rs",
        "AF_UNIX",
        "accepted_connections",
        "software_unix_shared_instance_local",
        "product_boundary",
        "not multi-process Python shared-instance interop evidence",
        "startup_status",
        "active",
        "attached",
        "runtime_iface",
        "shared_instance_type",
        "socket_path",
        "report.json",
    ] {
        assert!(
            script.contains(required),
            "LocalInterface Unix smoke should include required token {required:?}"
        );
    }
}

#[test]
fn local_interface_python_shared_smoke_preserves_interop_evidence_contract() {
    let root = repo_root();
    let script_path = root.join("tools/scripts/local-interface-python-shared-smoke.sh");
    let script =
        fs::read_to_string(&script_path).expect("read LocalInterface Python shared smoke script");

    for required in [
        "target/local-interface-python-shared-smoke",
        "RETICULUM_PY_REPO",
        "python_rns_revision",
        "RNS.Reticulum",
        "share_instance = yes",
        "shared_instance_type = tcp",
        "shared_instance_type = unix",
        "local-python-tcp-attach",
        "local-python-unix-attach",
        "LocalClientInterface",
        "socket_path = \"@rns/${PY_UNIX_INSTANCE}\"",
        "fixed_mtu = 262144",
        "force_shared_instance_bitrate = 1000000",
        "--strict-interface-startup",
        "start_reticulumd tcp",
        "start_reticulumd unix",
        "\"${RET_PID_TCP:-}\" \"${RET_PID_UNIX:-}\"",
        "kill -INT \"$RET_PID_UNIX\"",
        "wait \"$RET_PID_UNIX\"",
        "kill -INT \"$RET_PID_TCP\"",
        "wait \"$RET_PID_TCP\"",
        "TRAFFIC_STARTED=false",
        "[[ \"$TRAFFIC_STARTED\" == false ]]",
        "rnstatus-rs",
        "rnstatus_snapshot tcp",
        "rnstatus_snapshot unix",
        "bind private rpc unix socket",
        "local_client_count",
        "python_shared_instance_tcp_unix_attach_announce_payload_and_lifecycle",
        "python-traffic-tcp",
        "python-traffic-unix",
        "destination.announce",
        "codex-local-python-shared-payload:python-to-shared",
        "codex-local-python-shared-payload:shared-to-python",
        "received_payloads",
        "reverse_announced_count",
        "received_payloads.append(app_data.decode(\"utf-8\"))",
        "announced_count",
        "local_client_rxb_total",
        "local_client_txb_total",
        "unix_teardown_restart_verified",
        "tcp_teardown_restart_verified",
        "Python Unix shared instance did not observe reticulumd client teardown",
        "Python Unix shared instance did not reattach after reticulumd restart",
        "Python TCP shared instance did not observe reticulumd client teardown",
        "Python TCP shared instance did not reattach after reticulumd restart",
        "product_boundary",
        "one per pinned Python Reticulum shared instance",
        "\"daemons\"",
        "Python-origin announces move across the shared",
        "codex-local-python-shared-payload:python-to-shared",
        "codex-local-python-shared-payload:shared-to-python",
        "received_payloads",
        "reverse_announced_count",
        "does not prove broad application-level shared-instance traffic parity",
        "startup_status",
        "attached",
        "runtime_iface",
        "report.json",
    ] {
        assert!(
            script.contains(required),
            "LocalInterface Python shared smoke should include required token {required:?}"
        );
    }
}

#[test]
fn local_interface_python_shared_smoke_bind_check_matches_the_daemon_panic_message() {
    let root = repo_root();
    let script =
        fs::read_to_string(root.join("tools/scripts/local-interface-python-shared-smoke.sh"))
            .expect("read LocalInterface Python shared smoke script");
    let daemon = fs::read_to_string(
        root.join("crates/apps/reticulumd/src/bin/reticulumd/rpc_loop_parts/module_core.rs"),
    )
    .expect("read reticulumd rpc loop");

    let needle = "bind private rpc unix socket";
    assert!(
        script.contains(&format!("grep -q \"{needle}\"")),
        "the smoke should fail on the daemon's RPC unix bind message in a daemon log"
    );
    assert!(
        daemon.contains(&format!(".expect(\"{needle}\")")),
        "the daemon should still panic with the message the smoke greps for"
    );
}

#[test]
fn local_interface_python_shared_smoke_reports_daemon_paths_per_daemon() {
    let root = repo_root();
    let script =
        fs::read_to_string(root.join("tools/scripts/local-interface-python-shared-smoke.sh"))
            .expect("read LocalInterface Python shared smoke script");

    let helper = script
        .find("def daemon_report(rpc_addr, rpc_unix, db_path, rust_config_path, reticulumd_log, rnstatus_json, rnstatus_human):")
        .expect("the report builds one entry per daemon through daemon_report");
    let report = script.find("\nreport = {").expect("the report literal follows the helper");
    assert!(helper < report, "daemon_report should be defined before the report literal");
    assert!(script[report..].contains("\"daemons\": {"), "the report should carry a daemons group");
    for key in ["\"rust_config_path\": rust_config_path,", "\"reticulumd_log\": reticulumd_log,"] {
        let sites: Vec<usize> = script.match_indices(key).map(|(at, _)| at).collect();
        assert_eq!(
            sites.len(),
            1,
            "{key} should be written exactly once, inside daemon_report, not at the top level"
        );
        assert!(
            sites[0] > helper && sites[0] < report,
            "{key} should sit inside daemon_report, between the helper and the report literal"
        );
    }
}

#[test]
fn local_runbook_documents_tcp_shared_instance_smoke_artifacts() {
    let root = repo_root();
    let runbook_path = root.join("docs/runbooks/reticulumd-local-interface.md");
    let runbook = fs::read_to_string(&runbook_path).expect("read LocalInterface runbook");

    for required in [
        "Software TCP Shared-Instance Smoke",
        "./tools/scripts/local-interface-smoke.sh",
        "target/local-interface-smoke/",
        "`LocalInterface`",
        "`LocalClientInterface`",
        "`shared_instance_type = \"tcp\"`",
        "`fixed_mtu = 262144`",
        "`force_shared_instance_bitrate = 1000000`",
        "_runtime.startup_status = \"active\"",
        "_runtime.startup_status = \"attached\"",
        "fake shared instance accepting the attach connection",
        "multi-process Python shared-instance",
    ] {
        assert!(
            runbook.contains(required),
            "LocalInterface runbook should document smoke token {required:?}"
        );
    }
}

#[test]
fn local_runbook_documents_unix_shared_instance_smoke_artifacts() {
    let root = repo_root();
    let runbook_path = root.join("docs/runbooks/reticulumd-local-interface.md");
    let runbook = fs::read_to_string(&runbook_path).expect("read LocalInterface runbook");

    for required in [
        "Software Unix Shared-Instance Smoke",
        "./tools/scripts/local-interface-unix-smoke.sh",
        "target/local-interface-unix-smoke/",
        "`LocalInterface`",
        "`LocalClientInterface`",
        "`shared_instance_type = \"unix\"`",
        "filesystem Unix listener",
        "Linux abstract Unix listener",
        "Linux abstract Unix client attach",
        "_runtime.startup_status = \"active\"",
        "_runtime.startup_status = \"attached\"",
        "evidence_scope = \"software_unix_shared_instance_local\"",
        "product_boundary",
        "not multi-process Python shared-instance interop evidence",
    ] {
        assert!(
            runbook.contains(required),
            "LocalInterface runbook should document Unix smoke token {required:?}"
        );
    }
}

#[test]
fn local_runbook_documents_python_shared_instance_smoke_artifacts() {
    let root = repo_root();
    let runbook_path = root.join("docs/runbooks/reticulumd-local-interface.md");
    let runbook = fs::read_to_string(&runbook_path).expect("read LocalInterface runbook");

    for required in [
        "Pinned Python Shared-Instance Smoke",
        "./tools/scripts/local-interface-python-shared-smoke.sh",
        "target/local-interface-python-shared-smoke/",
        "`RETICULUM_PY_REPO`",
        "`LocalClientInterface`",
        "real pinned Python Reticulum shared instances",
        "TCP shared instance",
        "Linux abstract Unix shared instance",
        "_runtime.startup_status = \"attached\"",
        "python_rns_revision",
        "local_client_count",
        "announced_count",
        "local_client_rxb_total",
        "local_client_txb_total",
        "graceful daemon teardown and",
        "restart checks",
        "unix_teardown_restart_verified",
        "tcp_teardown_restart_verified",
        "one `reticulumd` per Python shared instance",
        "evidence_scope = \"python_shared_instance_tcp_unix_attach_announce_payload_and_lifecycle\"",
        "product_boundary",
        "exact announce payloads are observed between pinned",
        "does not prove daemon application-level packet consumption",
    ] {
        assert!(
            runbook.contains(required),
            "LocalInterface runbook should document Python shared smoke token {required:?}"
        );
    }
}
