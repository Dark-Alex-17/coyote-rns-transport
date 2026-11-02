// Usage probes for the lxmd smoke script's port defaults, written from the
// acceptance text rather than the script: "every `RUST_RPC_ADDR`,
// `RUST_TRANSPORT_ADDR`, `PY_SHARED_INSTANCE_PORT`, `PY_INSTANCE_CONTROL_PORT`,
// `PY_ENDPOINT_CONTROL_PORT` default is an OS-assigned free loopback port,
// pairwise distinct; env overrides still win", and the two Python control
// ports are picked "independently instead" of by offset. The sibling probe
// pins the script's text; these run its prologue (everything before the
// `PYTHON_PATHSEP=` line, which is where the first Python checkout is needed)
// and read the seven values the script derives, so a default that is merely
// spelled right but resolves to a taken, repeated or offset port is caught.

#[cfg(unix)]
const LXMD_SMOKE_PROLOGUE_STOP: &str = "PYTHON_PATHSEP=";

/// The seven values the prologue derives, in the order the smoke script
/// uses them.
#[cfg(unix)]
#[derive(Debug)]
struct LxmdSmokePorts {
    rpc_addr: String,
    transport_addr: String,
    transport_host: String,
    transport_port: String,
    shared_instance_port: String,
    instance_control_port: String,
    endpoint_control_port: String,
}

#[cfg(unix)]
impl LxmdSmokePorts {
    fn parse(line: &str) -> Self {
        let fields: Vec<&str> = line.split(' ').collect();
        assert_eq!(fields.len(), 7, "the prologue reports seven values, got {line:?}");
        Self {
            rpc_addr: fields[0].to_string(),
            transport_addr: fields[1].to_string(),
            transport_host: fields[2].to_string(),
            transport_port: fields[3].to_string(),
            shared_instance_port: fields[4].to_string(),
            instance_control_port: fields[5].to_string(),
            endpoint_control_port: fields[6].to_string(),
        }
    }

    /// The five ports the acceptance text names, as numbers.
    fn five_ports(&self) -> [u16; 5] {
        let port = |addr: &str, what: &str| -> u16 {
            addr.parse().unwrap_or_else(|err| panic!("{what} is a port number, got {addr:?}: {err}"))
        };
        let rpc = self.rpc_addr.rsplit(':').next().expect("rpc addr has a port");
        let transport = self.transport_addr.rsplit(':').next().expect("transport addr has a port");
        [
            port(rpc, "RUST_RPC_ADDR port"),
            port(transport, "RUST_TRANSPORT_ADDR port"),
            port(&self.shared_instance_port, "PY_SHARED_INSTANCE_PORT"),
            port(&self.instance_control_port, "PY_INSTANCE_CONTROL_PORT"),
            port(&self.endpoint_control_port, "PY_ENDPOINT_CONTROL_PORT"),
        ]
    }

    fn assert_pairwise_distinct(&self) {
        let mut ports = self.five_ports().to_vec();
        ports.sort_unstable();
        ports.dedup();
        assert_eq!(ports.len(), 5, "the five ports are pairwise distinct, got {self:?}");
    }
}

/// Runs the smoke script's prologue under `bash` with only `PATH` and `HOME`
/// carried over plus `extra`, and returns `(exit code, stdout, stderr)`. The
/// prologue is copied so a stray later line can never run a real daemon.
#[cfg(unix)]
fn run_lxmd_smoke_prologue(label: &str, extra: &[(&str, &str)]) -> (Option<i32>, String, String) {
    let script = read_repo_file(LXMD_SMOKE_SCRIPT);
    let mut prologue: Vec<&str> =
        script.lines().take_while(|line| !line.starts_with(LXMD_SMOKE_PROLOGUE_STOP)).collect();
    assert!(prologue.len() < script.lines().count(), "the smoke script has a {LXMD_SMOKE_PROLOGUE_STOP} line");
    prologue.push(
        "echo \"$RUST_RPC_ADDR $RUST_TRANSPORT_ADDR $RUST_TRANSPORT_HOST $RUST_TRANSPORT_PORT \
         $PY_SHARED_INSTANCE_PORT $PY_INSTANCE_CONTROL_PORT $PY_ENDPOINT_CONTROL_PORT\"",
    );
    let dir = std::env::temp_dir().join(format!("lxmd-smoke-prologue-{}-{label}", std::process::id()));
    fs::create_dir_all(&dir).expect("create the prologue scratch dir");
    let path = dir.join("prologue.sh");
    fs::write(&path, prologue.join("\n") + "\n").expect("write the prologue copy");

    let mut command = Command::new("bash");
    command
        .arg(&path)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", std::env::var("HOME").unwrap_or_default());
    for (key, value) in extra {
        command.env(key, value);
    }
    let output = command.output().expect("run the smoke script prologue under bash");
    let _ = fs::remove_dir_all(&dir);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[cfg(unix)]
fn lxmd_smoke_prologue_ports(label: &str, extra: &[(&str, &str)]) -> LxmdSmokePorts {
    let (code, stdout, stderr) = run_lxmd_smoke_prologue(label, extra);
    assert_eq!(code, Some(0), "the prologue exits 0; stdout {stdout:?} stderr {stderr:?}");
    LxmdSmokePorts::parse(stdout.trim_end_matches('\n'))
}

/// With nothing exported, the five defaults are loopback ports the OS handed
/// out: non-zero, pairwise distinct, bindable right now, and the transport
/// host/port split matches the transport address. A second cold run gets its
/// own set rather than a deterministic one.
#[cfg(unix)]
#[test]
fn lxmd_smoke_prologue_defaults_are_free_distinct_loopback_ports() {
    let first = lxmd_smoke_prologue_ports("defaults-1", &[]);
    let ports = first.five_ports();
    assert!(first.rpc_addr.starts_with("127.0.0.1:"), "RUST_RPC_ADDR is loopback, got {first:?}");
    assert!(first.transport_addr.starts_with("127.0.0.1:"), "RUST_TRANSPORT_ADDR is loopback, got {first:?}");
    assert_eq!(first.transport_host, "127.0.0.1", "RUST_TRANSPORT_HOST is the address's host");
    assert_eq!(first.transport_port, ports[1].to_string(), "RUST_TRANSPORT_PORT is the address's port");
    assert!(ports.iter().all(|port| *port != 0), "every default is a real port, got {first:?}");
    first.assert_pairwise_distinct();
    for port in ports {
        std::net::TcpListener::bind(("127.0.0.1", port))
            .unwrap_or_else(|err| panic!("default port {port} is free to bind on loopback: {err}"));
    }

    let second = lxmd_smoke_prologue_ports("defaults-2", &[]);
    assert_ne!(
        second.five_ports(),
        ports,
        "a second run asks the OS again instead of recomputing a fixed set"
    );
}

/// An exported port wins verbatim and the remaining defaults are still picked
/// by the OS and stay distinct from each other and from the pinned ones.
#[cfg(unix)]
#[test]
fn lxmd_smoke_prologue_keeps_exported_ports_and_picks_the_rest() {
    let ports = lxmd_smoke_prologue_ports(
        "two-pinned",
        &[("RUST_RPC_ADDR", "127.0.0.1:5555"), ("PY_INSTANCE_CONTROL_PORT", "6666")],
    );
    assert_eq!(ports.rpc_addr, "127.0.0.1:5555", "the exported RUST_RPC_ADDR is kept verbatim");
    assert_eq!(ports.instance_control_port, "6666", "the exported PY_INSTANCE_CONTROL_PORT is kept verbatim");
    assert!(ports.transport_addr.starts_with("127.0.0.1:"), "RUST_TRANSPORT_ADDR is still a loopback default");
    ports.assert_pairwise_distinct();

    let all = lxmd_smoke_prologue_ports(
        "all-pinned",
        &[
            ("RUST_RPC_ADDR", "127.0.0.1:5555"),
            ("RUST_TRANSPORT_ADDR", "10.0.0.9:5556"),
            ("PY_SHARED_INSTANCE_PORT", "5557"),
            ("PY_INSTANCE_CONTROL_PORT", "5558"),
            ("PY_ENDPOINT_CONTROL_PORT", "5559"),
        ],
    );
    assert_eq!(all.rpc_addr, "127.0.0.1:5555");
    assert_eq!(all.transport_addr, "10.0.0.9:5556", "a non-loopback transport address is kept as given");
    assert_eq!(all.transport_host, "10.0.0.9", "RUST_TRANSPORT_HOST follows the exported address");
    assert_eq!(all.transport_port, "5556", "RUST_TRANSPORT_PORT follows the exported address");
    assert_eq!(
        [all.shared_instance_port.as_str(), all.instance_control_port.as_str(), all.endpoint_control_port.as_str()],
        ["5557", "5558", "5559"],
        "every exported Python port is kept verbatim"
    );
}

/// The two Python control ports are OS-assigned in their own right: pinning
/// the shared-instance port no longer makes them `+1` and `+11` of it. This is
/// the arm that fails against the pid-derived script, whose control ports
/// were offsets of `PY_SHARED_INSTANCE_PORT`. The pinned port sits below every
/// OS ephemeral range (Linux hands out 32768-60999) so a control port the OS
/// picks can never land on one of the offsets by chance.
#[cfg(unix)]
#[test]
fn lxmd_smoke_prologue_picks_control_ports_independently_of_the_shared_instance_port() {
    let ports = lxmd_smoke_prologue_ports("shared-pinned", &[("PY_SHARED_INSTANCE_PORT", "7000")]);
    assert_eq!(ports.shared_instance_port, "7000", "the exported shared-instance port is kept");
    assert_ne!(ports.instance_control_port, "7001", "PY_INSTANCE_CONTROL_PORT is not shared-instance + 1");
    assert_ne!(ports.endpoint_control_port, "7011", "PY_ENDPOINT_CONTROL_PORT is not shared-instance + 11");
    assert_ne!(ports.endpoint_control_port, "7010", "PY_ENDPOINT_CONTROL_PORT is not shared-instance + 10");
    ports.assert_pairwise_distinct();
}

/// A prologue that cannot run its Python helper stops loudly instead of
/// carrying empty ports into the daemons' configs.
#[cfg(unix)]
#[test]
fn lxmd_smoke_prologue_fails_loudly_without_a_python() {
    let (code, stdout, stderr) = run_lxmd_smoke_prologue("no-python", &[("PYTHON_BIN", "/nonexistent/python3")]);
    assert_ne!(code, Some(0), "the prologue fails when its Python helper cannot run; stderr {stderr:?}");
    assert!(
        !stdout.contains("127.0.0.1:"),
        "no address with an empty or guessed port is reported, got stdout {stdout:?}"
    );
    assert!(stderr.contains("/nonexistent/python3"), "the failure names the missing interpreter, got {stderr:?}");
}
