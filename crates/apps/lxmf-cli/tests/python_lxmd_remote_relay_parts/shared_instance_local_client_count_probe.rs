// Spec-first pins for the live re-attach observable the router-restart test
// now waits on: the Python control endpoint's `status` reports
// `reticulum.local_client_count` as the number of local clients the shared
// instance currently holds, so it reads 0 before a Rust `local_client`
// attaches, 1 once the daemon is attached (the moment the Rust-side static
// `startup_status` check also passes), and 0 again after the daemon exits.
// The companion test pins the helper's failure shape: against an endpoint
// that never reports the expected count it returns Err within its 60 s
// bound naming the label, the count and the port, rather than running to
// the 300 s path timeout the test used to hit.

/// Polls `status` until `reticulum.local_client_count` reads `expected`
/// exactly, or the bound elapses.
fn wait_for_python_local_client_count_exactly(
    control_port: u16,
    expected: u64,
    bound: Duration,
) -> Result<Value, String> {
    let deadline = Instant::now() + bound;
    let mut last = Value::Null;
    while Instant::now() < deadline {
        let status = python_control_call(control_port, "status", None)?;
        if status["reticulum"]["local_client_count"].as_u64() == Some(expected) {
            return Ok(status);
        }
        last = status;
        thread::sleep(Duration::from_millis(250));
    }
    Err(format!("local_client_count did not read {expected} within {bound:?}; last status: {last}"))
}

#[test]
#[ignore = "requires local Python Reticulum/LXMF repos and daemon runtime"]
fn python_shared_instance_status_local_client_count_tracks_the_live_attachment() {
    let lxmd_bin = resolve_test_binary("lxmd", option_env!("CARGO_BIN_EXE_lxmd"));
    let reticulumd_bin = resolve_test_binary("reticulumd", option_env!("CARGO_BIN_EXE_reticulumd"));
    let workspace_root =
        Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).expect("workspace root");
    let python_bin = env::var("LXMF_PYTHON_BIN").unwrap_or_else(|_| "python3".to_string());
    let reticulum_repo = env::var("RETICULUM_PY_REPO").unwrap_or_else(|_| {
        workspace_root.parent().expect("workspace parent").join("reticulum").display().to_string()
    });
    let lxmf_repo = env::var("LXMF_PY_REPO").unwrap_or_else(|_| {
        workspace_root.parent().expect("workspace parent").join("lxmf").display().to_string()
    });
    let helper_script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("support")
        .join("python_lxmf_endpoint.py");
    assert!(Path::new(&reticulum_repo).exists(), "reticulum repo not found: {reticulum_repo}");
    assert!(Path::new(&lxmf_repo).exists(), "lxmf repo not found: {lxmf_repo}");
    assert!(helper_script.exists(), "Python helper script missing: {}", helper_script.display());

    let temp = tempfile::tempdir().expect("tempdir");
    let shared_instance = ReservedPort::reserve();
    let rust_rpc = ReservedPort::reserve();
    let python_control = ReservedPort::reserve();
    let rns_control = ReservedPort::reserve();
    let shared_instance_port = shared_instance.port();
    let rust_rpc_port = rust_rpc.port();
    let python_control_port = python_control.port();
    let rns_control_port = rns_control.port();
    let rust_dir = temp.path().join("rust-client");
    let python_storage = temp.path().join("python-owner-storage");
    let python_rns = temp.path().join("python-owner-rns");
    write_rust_config(
        &rust_dir,
        &rust_node_config(
            "rust-client",
            rust_rpc_port,
            None,
            &[local_client_interface("python-shared", shared_instance_port)],
        ),
    );
    write_python_shared_instance_rns_config_with_control_port(
        &python_rns,
        shared_instance_port,
        rns_control_port,
    );

    let mut python_node = Some(spawn_python_endpoint(
        &python_bin,
        &reticulum_repo,
        &lxmf_repo,
        &helper_script,
        "python-owner",
        "Python shared-instance owner",
        &python_rns,
        &python_storage,
        python_control_port,
        &mut [python_control, shared_instance, rns_control],
    ));
    let mut rust_node = None;
    let outcome: Result<(), String> = (|| {
        wait_for_python_endpoint_ready(
            python_control_port,
            python_node.as_mut().expect("Python owner"),
            "Python shared-instance owner",
        )?;
        // Before any Rust client attaches: the key is present, integral, zero.
        let idle = python_control_call(python_control_port, "status", None)?;
        let reticulum = idle
            .get("reticulum")
            .ok_or_else(|| format!("status has no reticulum block: {idle}"))?;
        if reticulum.get("is_shared_instance") != Some(&Value::Bool(true)) {
            return Err(format!("Python owner is not a shared instance: {idle}"));
        }
        match reticulum.get("local_client_count") {
            Some(Value::Number(count)) if count.as_u64() == Some(0) => {}
            other => {
                return Err(format!(
                    "local_client_count should be the integer 0 before any client attaches, \
                     got {other:?} in {idle}"
                ))
            }
        }
        if wait_for_python_local_client_count(python_control_port, 0, "idle owner").is_err() {
            return Err("the helper accepts an expected count of 0 on an idle owner".to_string());
        }

        rust_node =
            Some(spawn_lxmd(&lxmd_bin, &reticulumd_bin, rust_rpc_port, &rust_dir, &mut [rust_rpc]));
        wait_for_ready(rust_rpc_port, rust_node.as_mut().expect("Rust client"), "Rust client")?;
        require_attached_local_client(rust_rpc_port, "Rust client")?;
        // The static Rust-side check and the live Python-side count agree.
        wait_for_python_local_client_count(python_control_port, 1, "owner with one Rust client")?;
        let attached = wait_for_python_local_client_count_exactly(
            python_control_port,
            1,
            Duration::from_secs(10),
        )?;
        if attached["reticulum"]["local_client_count"].as_u64() != Some(1) {
            return Err(format!("expected exactly one attached local client: {attached}"));
        }

        // After the Rust daemon exits the shared instance drops the spawned
        // LocalClientInterface and the count falls back to 0: the observable
        // is live, not a static attach record.
        terminate_child(&mut rust_node.as_mut().expect("Rust client").child);
        wait_for_python_local_client_count_exactly(
            python_control_port,
            0,
            Duration::from_secs(30),
        )?;
        Ok(())
    })();

    let failure_details = outcome.as_ref().err().map(|error| {
        format!(
            "{error}\n\n{}\n\n{}",
            collect_node_diagnostics("Rust client", rust_rpc_port, rust_node.as_mut()),
            collect_python_endpoint_diagnostics(
                "Python shared-instance owner",
                python_control_port,
                python_node.as_mut(),
            ),
        )
    });
    if let Some(node) = rust_node.as_mut() {
        terminate_child(&mut node.child);
    }
    if let Some(node) = python_node.as_mut() {
        terminate_child(&mut node.child);
    }
    if let Some(details) = failure_details {
        panic!("Python shared-instance local_client_count probe failed:\n{details}");
    }
}

/// A stand-in control endpoint that answers every request with a fixed
/// `status` result and closes the connection, like the Python helper does.
fn spawn_fixed_status_endpoint(
    result: Value,
) -> (u16, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, BufReader};
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("bind fake control endpoint");
    let port = listener.local_addr().expect("fake endpoint addr").port();
    let served = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&served);
    let body = serde_json::to_vec(&json!({ "ok": true, "result": result })).expect("encode");
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut line = String::new();
            let _ = BufReader::new(stream.try_clone().expect("clone")).read_line(&mut line);
            let _ = std::io::Write::write_all(&mut stream, &body);
            let _ = std::io::Write::write_all(&mut stream, b"\n");
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    });
    (port, served)
}

#[test]
#[ignore = "takes the helper's full 60 s bound to prove the failure shape"]
fn wait_for_python_local_client_count_fails_within_its_bound_when_no_client_attaches() {
    let (port, served) =
        spawn_fixed_status_endpoint(json!({ "reticulum": { "local_client_count": 0 } }));
    // A status without the key (the pre-fix endpoint) must fail the same
    // way instead of being mistaken for an attached client, and a count
    // above the expectation satisfies the wait; all three run concurrently
    // so the test pays the 60 s bound once.
    let (missing_port, _) = spawn_fixed_status_endpoint(json!({ "reticulum": {} }));
    let (ok_port, _) =
        spawn_fixed_status_endpoint(json!({ "reticulum": { "local_client_count": 2 } }));
    let keyless = thread::spawn(move || {
        let started = Instant::now();
        (wait_for_python_local_client_count(missing_port, 1, "keyless owner"), started.elapsed())
    });
    let satisfied = thread::spawn(move || {
        let started = Instant::now();
        (wait_for_python_local_client_count(ok_port, 1, "owner with two"), started.elapsed())
    });
    let started = Instant::now();
    let result = wait_for_python_local_client_count(port, 1, "fixed-zero owner");
    let elapsed = started.elapsed();
    let err = result.expect_err("an owner that never reports a client makes the helper fail");
    assert!(
        elapsed >= Duration::from_secs(60) && elapsed < Duration::from_secs(75),
        "the helper gives up at its 60 s bound, not the 300 s path timeout: took {elapsed:?}"
    );
    for needle in ["fixed-zero owner", "at least 1", &format!("control port {port}"), "last status"]
    {
        assert!(err.contains(needle), "the error names {needle:?}, got {err:?}");
    }
    assert!(err.contains("\"local_client_count\":0"), "the error carries the last status: {err}");
    let polls = served.load(std::sync::atomic::Ordering::SeqCst);
    assert!(polls >= 60, "the helper polls on its 500 ms interval, saw {polls} requests");

    let (satisfied, quickly) = satisfied.join().expect("satisfied waiter");
    assert!(satisfied.is_ok(), "a count above the expectation satisfies the wait: {satisfied:?}");
    assert!(quickly < Duration::from_secs(5), "and does so on the first poll, took {quickly:?}");
    let (keyless, keyless_elapsed) = keyless.join().expect("keyless waiter");
    let err = keyless.expect_err("a status without the key never satisfies the wait");
    assert!(keyless_elapsed >= Duration::from_secs(60), "the keyless case also runs to the bound");
    assert!(err.contains("keyless owner"), "{err}");
}
