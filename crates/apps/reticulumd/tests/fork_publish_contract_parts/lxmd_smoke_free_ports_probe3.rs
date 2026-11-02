// Usage probes closing the loop between the lxmd smoke script's OS-assigned
// port defaults and the daemon that binds them. The failure the fix removes
// was a `reticulumd` panic at `rpc_loop_parts/network_listeners.rs:12` when
// the pid-derived RPC port was already taken; the fix moved the ports and left
// that bind loud. The sibling probes pin the script's text and its prologue;
// these run the real `reticulumd` binary: once on the RPC address the
// prologue derives (it must come up listening there), and once on a loopback
// port another process already holds (it must still die loudly, naming the
// bind).

#[cfg(unix)]
struct LxmdSmokeDaemon(std::process::Child);

#[cfg(unix)]
impl Drop for LxmdSmokeDaemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Starts `reticulumd` with only a TCP RPC bind and a scratch database, the
/// two inputs the smoke script's prologue decides, with stderr captured. The
/// private Unix RPC socket gets a short `/tmp` path: a macOS `temp_dir()`
/// path overruns `SUN_LEN` and that bind would fail for its own reason.
#[cfg(unix)]
fn start_lxmd_smoke_daemon(label: &str, rpc_addr: &str) -> (LxmdSmokeDaemon, PathBuf) {
    let dir = std::env::temp_dir().join(format!("lxmd-smoke-daemon-{}-{label}", std::process::id()));
    fs::create_dir_all(&dir).expect("create the daemon scratch dir");
    let socket = PathBuf::from(format!("/tmp/lxmd-smoke-{}-{label}.sock", std::process::id()));
    let _ = fs::remove_file(&socket);
    let child = Command::new(env!("CARGO_BIN_EXE_reticulumd"))
        .args(["--rpc", rpc_addr])
        .arg("--db")
        .arg(dir.join("reticulum.db"))
        .arg("--identity")
        .arg(dir.join("identity"))
        .arg("--rpc-unix")
        .arg(&socket)
        .env("TOKIO_WORKER_THREADS", "2")
        .env("RUST_LOG", "error")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("start reticulumd on the smoke prologue's RPC address");
    (LxmdSmokeDaemon(child), dir)
}

/// The RPC address the smoke prologue hands `reticulumd` is one the daemon
/// can actually bind: it comes up listening there, and the port is still the
/// one the prologue printed (nothing re-derives it on the way in).
#[cfg(unix)]
#[test]
fn lxmd_smoke_prologue_rpc_address_is_bindable_by_reticulumd() {
    let ports = lxmd_smoke_prologue_ports("daemon-binds", &[]);
    let rpc_addr = ports.rpc_addr.clone();
    let (mut daemon, dir) = start_lxmd_smoke_daemon("binds", &rpc_addr);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let listening = loop {
        if std::net::TcpStream::connect(&rpc_addr).is_ok() {
            break true;
        }
        if let Some(status) = daemon.0.try_wait().expect("poll reticulumd") {
            let mut stderr = String::new();
            if let Some(mut pipe) = daemon.0.stderr.take() {
                let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
            }
            panic!("reticulumd exited {status} before listening on {rpc_addr}; stderr {stderr:?}");
        }
        if std::time::Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert!(listening, "reticulumd listens on the prologue's RUST_RPC_ADDR {rpc_addr} within 30 s");
    let _ = daemon.0.kill();
    let _ = daemon.0.wait();
    let mut stderr = String::new();
    if let Some(mut pipe) = daemon.0.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
    }
    assert!(!stderr.contains("panicked"), "a daemon on the prologue's ports starts without a panic, got {stderr:?}");
    let _ = fs::remove_file(format!("/tmp/lxmd-smoke-{}-binds.sock", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
}

/// A loopback port another process already holds still makes `reticulumd`
/// die at startup, loudly: a non-zero exit whose stderr names the RPC
/// listener bind and the address-in-use cause. This is the failure the smoke
/// script used to trip on a busy runner; the fix moved the ports, not the
/// panic.
#[cfg(unix)]
#[test]
fn reticulumd_still_fails_loudly_when_the_rpc_port_is_taken() {
    let holder = std::net::TcpListener::bind(("127.0.0.1", 0)).expect("hold a loopback port");
    let taken = holder.local_addr().expect("held port address");
    let (mut daemon, dir) = start_lxmd_smoke_daemon("taken", &taken.to_string());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(status) = daemon.0.try_wait().expect("poll reticulumd") {
            break status;
        }
        assert!(std::time::Instant::now() < deadline, "reticulumd exits instead of running on a taken RPC port");
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let mut stderr = String::new();
    if let Some(mut pipe) = daemon.0.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut pipe, &mut stderr);
    }
    assert!(!status.success(), "a taken RPC port is fatal, got {status}; stderr {stderr:?}");
    assert!(stderr.contains("bind rpc listener"), "the failure names the RPC listener bind, got {stderr:?}");
    assert!(stderr.contains("AddrInUse"), "the failure carries the address-in-use cause, got {stderr:?}");
    drop(holder);
    let _ = fs::remove_file(format!("/tmp/lxmd-smoke-{}-taken.sock", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
}
