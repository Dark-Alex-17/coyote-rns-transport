// The graceful stop the restart part relies on, pinned without Python: the
// contract of `terminate_child_gracefully` (`tests/support/python_channel_process.rs`)
// as the restart proof uses it through `SharedInstanceRpcRnsd::stop_gracefully`.
//
//  * the child gets SIGTERM, not SIGKILL, so its TERM handler runs (rnsd's
//    exit handler is where the path table is persisted), and the clean exit
//    status comes back once the child is reaped;
//  * a child that ignores TERM is killed and reaped inside the bound plus a
//    little, and the failure is a panic naming the bound, never a hang on
//    the pipes or a leaked process;
//  * a child that exits non-zero on TERM is reported as a failed shutdown,
//    and is reaped;
//  * a `ChildGuard` whose child was taken for the graceful stop does nothing
//    on drop, so the stop is never followed by a second kill of a reaped pid.
//
// `/bin/sh` stands in for rnsd: a `trap` on TERM is the exit handler, a
// marker file its persisted table, and a `ready` file written right after
// the trap is installed tells the test the signal can land. Unix only like
// the helper.

#[cfg(unix)]
const GRACEFUL_WITHIN: Duration = Duration::from_secs(5);

/// A shell child that installs the TERM trap, writes `ready`, then loops
/// until TERM, when it runs `on_term`.
#[cfg(unix)]
fn graceful_probe_child(ready: &Path, on_term: &str) -> std::process::Child {
    std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "trap '{on_term}' TERM; : > {}; while :; do sleep 0.05; done",
                ready.display()
            ),
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn /bin/sh")
}

/// Waits for the child's `ready` file so the signal lands on an installed trap.
#[cfg(unix)]
fn wait_until_ready(ready: &Path) {
    let started = std::time::Instant::now();
    while !ready.exists() {
        assert!(
            started.elapsed() < GRACEFUL_WITHIN,
            "{} not written within {GRACEFUL_WITHIN:?}",
            ready.display()
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Runs the helper under `catch_unwind` and returns the panic message.
#[cfg(unix)]
fn graceful_stop_panics_with(child: &mut std::process::Child, within: Duration) -> String {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        terminate_child_gracefully(child, within)
    }));
    let payload = outcome.expect_err("the graceful stop must fail");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .expect("a string panic payload")
}

#[cfg(unix)]
#[test]
fn terminate_child_gracefully_lets_the_term_handler_run_and_returns_the_clean_exit() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ready = temp.path().join("ready");
    let marker = temp.path().join("handled");
    let mut child =
        graceful_probe_child(&ready, &format!("echo handled > {}; exit 0", marker.display()));
    wait_until_ready(&ready);
    let started = std::time::Instant::now();
    let status = terminate_child_gracefully(&mut child, GRACEFUL_WITHIN);
    let elapsed = started.elapsed();
    assert!(status.success(), "a child that exits 0 on TERM: {status}");
    assert!(elapsed < GRACEFUL_WITHIN, "returned {elapsed:?} after the signal");
    assert_eq!(
        fs::read_to_string(&marker).expect("the TERM handler wrote the marker").trim(),
        "handled",
        "SIGTERM, not SIGKILL: the handler ran"
    );
    assert!(
        matches!(child.try_wait(), Ok(Some(reaped)) if reaped.success()),
        "the child is reaped, not left for the caller"
    );
}

#[cfg(unix)]
#[test]
fn terminate_child_gracefully_kills_and_reaps_a_child_that_ignores_term_inside_the_bound() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ready = temp.path().join("ready");
    let within = Duration::from_millis(500);
    let mut child = graceful_probe_child(&ready, "");
    wait_until_ready(&ready);
    let started = std::time::Instant::now();
    let message = graceful_stop_panics_with(&mut child, within);
    let elapsed = started.elapsed();
    assert!(
        message.contains("did not exit within") && message.contains("500ms"),
        "the panic names the bound: {message}"
    );
    assert!(elapsed < within + Duration::from_secs(2), "bounded: {elapsed:?}");
    assert!(
        matches!(child.try_wait(), Ok(Some(_))),
        "a stubborn child is killed and reaped before the panic, never leaked"
    );
}

#[cfg(unix)]
#[test]
fn terminate_child_gracefully_reports_a_nonzero_exit_on_term_as_a_failed_shutdown() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ready = temp.path().join("ready");
    let mut child = graceful_probe_child(&ready, "exit 3");
    wait_until_ready(&ready);
    let message = graceful_stop_panics_with(&mut child, GRACEFUL_WITHIN);
    assert!(
        message.contains("failed during graceful shutdown") && message.contains("exit status: 3"),
        "the panic carries the exit status: {message}"
    );
    assert!(matches!(child.try_wait(), Ok(Some(_))), "the child is reaped");
}

#[cfg(unix)]
#[test]
fn child_guard_does_not_kill_again_after_its_child_was_taken_for_a_graceful_stop() {
    let temp = tempfile::tempdir().expect("tempdir");
    let ready = temp.path().join("ready");
    let marker = temp.path().join("handled");
    let mut guard = ChildGuard {
        child: Some(graceful_probe_child(
            &ready,
            &format!("echo handled > {}; exit 0", marker.display()),
        )),
    };
    wait_until_ready(&ready);
    // The restart harness's order: take, stop gracefully, let the guard drop.
    let mut child = guard.child.take().expect("the guard held the child");
    let status = terminate_child_gracefully(&mut child, GRACEFUL_WITHIN);
    assert!(status.success(), "{status}");
    assert!(guard.child.is_none(), "the guard has nothing left to signal");
    // Drop finds `None` and so never signals the pid the stop already reaped.
    drop(guard);
    assert_eq!(fs::read_to_string(&marker).expect("marker").trim(), "handled");
}
