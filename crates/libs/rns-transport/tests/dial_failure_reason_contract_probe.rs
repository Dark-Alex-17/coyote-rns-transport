//! The dial-failure reason types as a downstream crate meets them, and the first-connect
//! timeout over a unix path. `DialFailureKind` and `DisconnectReason` are both
//! `#[non_exhaustive]` and `Debug + Clone + Copy + PartialEq + Eq`; `DialFailure` has
//! private fields read through getters; and, as the CHANGELOG's `DialFailed` bullet says,
//! `SharedInstanceEvent` lost its numeric cast once a variant carried data. From outside the
//! crate:
//!
//! - a downstream `match` on `DialFailureKind` and on `DisconnectReason` needs the wildcard
//!   arm `#[non_exhaustive]` forces (`#[deny(unreachable_patterns)]` turns a dropped
//!   attribute into a compile error), and both enums are `Eq + Debug`;
//! - driving `rustc` against the linked rlib the way `breaking_changes_compile_fail.rs`
//!   does: the migrated consumer shapes compile, while `SharedInstanceEvent::Connected as
//!   isize`, a `DialFailure { .. }` struct literal, and exhaustive matches on either reason
//!   enum without a wildcard are rejected with the diagnostic a consumer sees;
//! - `with_first_connect_timeout` over `SharedInstanceEndpoint::Unix`, which the other pins
//!   hold for TCP only: against a tempdir path nothing has bound the attach returns
//!   `Err(SharedInstanceConnectTimeout)`, `shared_instance()` is `None`, the posture is off,
//!   the stream reads `Connecting`, one or more `DialFailed { NotFound }`, then ends, never
//!   `Connected`; once a `LocalUnixServer` binds the path the same transport attaches again
//!   with the timeout satisfied and reads `Connecting`, `Connected`.

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rns_transport::transport::{DialFailureKind, DisconnectReason};

/// The shape every downstream `match` on `DialFailureKind` has to take: a wildcard arm.
#[deny(unreachable_patterns)]
fn dial_failure_name(kind: DialFailureKind) -> &'static str {
    match kind {
        DialFailureKind::Timeout => "timeout",
        DialFailureKind::Refused => "refused",
        DialFailureKind::NotFound => "not_found",
        DialFailureKind::Io(_) => "io",
        _ => "unknown",
    }
}

/// The same for `DisconnectReason`, which no client emits yet but a consumer may already
/// match on.
#[deny(unreachable_patterns)]
fn disconnect_name(reason: DisconnectReason) -> &'static str {
    match reason {
        DisconnectReason::PeerClosed => "peer_closed",
        DisconnectReason::Reset => "reset",
        DisconnectReason::Timeout => "timeout",
        DisconnectReason::Io(_) => "io",
        _ => "unknown",
    }
}

#[test]
fn the_reason_enums_are_non_exhaustive_eq_and_debug_outside_the_crate() {
    fn is_eq<T: Eq>() {}
    fn is_debug<T: Debug>() {}
    is_eq::<DialFailureKind>();
    is_eq::<DisconnectReason>();
    is_debug::<DialFailureKind>();
    is_debug::<DisconnectReason>();

    assert_eq!(dial_failure_name(DialFailureKind::Timeout), "timeout");
    assert_eq!(dial_failure_name(DialFailureKind::Refused), "refused");
    assert_eq!(dial_failure_name(DialFailureKind::NotFound), "not_found");
    assert_eq!(dial_failure_name(DialFailureKind::Io(std::io::ErrorKind::Other)), "io");
    assert_eq!(disconnect_name(DisconnectReason::PeerClosed), "peer_closed");
    assert_eq!(disconnect_name(DisconnectReason::Reset), "reset");
    assert_eq!(disconnect_name(DisconnectReason::Timeout), "timeout");
    assert_eq!(disconnect_name(DisconnectReason::Io(std::io::ErrorKind::Other)), "io");
    assert_eq!(
        format!("{:?}", DialFailureKind::Io(std::io::ErrorKind::PermissionDenied)),
        "Io(PermissionDenied)"
    );
}

/// The migrated shapes the CHANGELOG bullet tells a consumer to write: a match on the
/// `DialFailed { reason }` variant with the wildcard arm, and a `DialFailure` read through
/// its getters. Compiling this first proves the harness works against the chosen rlib.
const MIGRATED_CONSUMER: &str = r#"
use rns_transport::transport::{DialFailure, DialFailureKind, DisconnectReason, SharedInstanceEvent};

pub fn reason(event: &SharedInstanceEvent) -> Option<DialFailureKind> {
    match event {
        SharedInstanceEvent::DialFailed { reason } => Some(*reason),
        SharedInstanceEvent::Connected => None,
        _ => None,
    }
}

pub fn refused(failure: &DialFailure) -> bool {
    failure.kind() == DialFailureKind::Refused && failure.iface() == failure.iface()
}

pub fn ended_by_peer(reason: DisconnectReason) -> bool {
    match reason {
        DisconnectReason::PeerClosed => true,
        _ => false,
    }
}
"#;

/// Each shape the reason types reject, with the diagnostic fragment a consumer sees.
/// rustc words the private-field error by snippet shape: a struct literal lists "fields
/// `a` and `b` of struct `S` are private", a field access says "field `a` of struct `S` is
/// private"; the fragment below is the part the struct-literal shape keeps.
const REJECTED: &[(&str, &str, &str)] = &[
    (
        "shared_instance_event_as_isize",
        "use rns_transport::transport::SharedInstanceEvent;\n\
         pub fn code() -> isize { SharedInstanceEvent::Connected as isize }\n",
        "non-primitive cast",
    ),
    (
        "dial_failure_struct_literal",
        "use rns_transport::hash::AddressHash;\n\
         use rns_transport::transport::{DialFailure, DialFailureKind};\n\
         pub fn build(iface: AddressHash) -> DialFailure {\n\
             DialFailure { iface, kind: DialFailureKind::Refused }\n\
         }\n",
        "of struct `DialFailure` are private",
    ),
    (
        "dial_failure_kind_match_without_wildcard",
        "use rns_transport::transport::DialFailureKind;\n\
         pub fn name(kind: DialFailureKind) -> &'static str {\n\
             match kind {\n\
                 DialFailureKind::Timeout => \"timeout\",\n\
                 DialFailureKind::Refused => \"refused\",\n\
                 DialFailureKind::NotFound => \"not_found\",\n\
                 DialFailureKind::Io(_) => \"io\",\n\
             }\n\
         }\n",
        "non-exhaustive patterns",
    ),
    (
        "disconnect_reason_match_without_wildcard",
        "use rns_transport::transport::DisconnectReason;\n\
         pub fn name(reason: DisconnectReason) -> &'static str {\n\
             match reason {\n\
                 DisconnectReason::PeerClosed => \"peer_closed\",\n\
                 DisconnectReason::Reset => \"reset\",\n\
                 DisconnectReason::Timeout => \"timeout\",\n\
                 DisconnectReason::Io(_) => \"io\",\n\
             }\n\
         }\n",
        "non-exhaustive patterns",
    ),
];

/// Scratch directory removed when the test ends, panic included.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rns-transport-dial-failure-reason-compile-fail-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("main")
        ));
        fs::create_dir_all(&dir).expect("scratch directory should be creatable");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The `deps` directory this test binary was linked from.
fn deps_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary should know its own path");
    exe.parent().map(Path::to_path_buf).expect("test binary should live in target/<profile>/deps")
}

/// Every `librns_transport-*.rlib` in the deps directory, newest first.
fn candidate_rlibs(deps: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(deps)
        .expect("deps directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
                name.starts_with("librns_transport-") && name.ends_with(".rlib")
            })
        })
        .filter_map(|path| {
            let modified = fs::metadata(&path).and_then(|meta| meta.modified()).ok()?;
            Some((modified, path))
        })
        .collect();
    found.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    found.into_iter().map(|(_, path)| path).collect()
}

fn rustc(scratch: &Path, deps: &Path, rlib: &Path, name: &str, source: &str) -> Output {
    let file = scratch.join(format!("{name}.rs"));
    fs::write(&file, source).expect("snippet source should be writable");
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    Command::new(compiler)
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--crate-name")
        .arg(name)
        .arg("-L")
        .arg(format!("dependency={}", deps.display()))
        .arg("--extern")
        .arg(format!("rns_transport={}", rlib.display()))
        .arg("--out-dir")
        .arg(scratch)
        .arg(&file)
        .output()
        .expect("rustc should be runnable from a cargo test")
}

/// The rlib the migrated consumer compiles against, with the deps directory.
fn working_rlib(scratch: &Path) -> (PathBuf, PathBuf) {
    let deps = deps_dir();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib next to {}", deps.display());
    let mut failures = Vec::new();
    for rlib in candidates {
        let output =
            rustc(scratch, &deps, &rlib, "dial_failure_migrated_consumer", MIGRATED_CONSUMER);
        if output.status.success() {
            return (deps, rlib);
        }
        failures.push(format!("{}:\n{}", rlib.display(), String::from_utf8_lossy(&output.stderr)));
    }
    panic!(
        "the consumer shapes the CHANGELOG describes should compile against the crate's rlib; \
         every candidate failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn the_cast_the_changelog_retires_and_the_private_shapes_do_not_compile() {
    let scratch = Scratch::new();
    let (deps, rlib) = working_rlib(&scratch.0);
    let mut unexpected = Vec::new();
    for (name, source, promised) in REJECTED {
        let output = rustc(&scratch.0, &deps, &rlib, name, source);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            unexpected.push(format!("{name}: compiled, but the contract says it does not"));
        } else if !stderr.contains(promised) {
            unexpected.push(format!(
                "{name}: failed, but not with the promised diagnostic {promised:?}:\n{stderr}"
            ));
        }
    }
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n\n"));
}

#[cfg(unix)]
mod unix_first_connect {
    use std::time::{Duration, Instant};

    use rns_transport::error::RnsError;
    use rns_transport::identity::PrivateIdentity;
    use rns_transport::iface::local::{LocalUnixEndpoint, LocalUnixServer};
    use rns_transport::rand_core::OsRng;
    use rns_transport::transport::{
        DialFailureKind, SharedInstanceEndpoint, SharedInstanceEvent, SharedInstanceOptions,
        Transport, TransportConfig,
    };
    use tokio::sync::mpsc;
    use tokio::time::timeout;

    const WAIT: Duration = Duration::from_secs(15);
    const POLL: Duration = Duration::from_millis(20);
    const RECONNECT_WAIT: Duration = Duration::from_millis(50);
    const FIRST_CONNECT: Duration = Duration::from_millis(300);

    #[deny(clippy::await_holding_invalid_type)]
    async fn next_event(
        events: &mut mpsc::Receiver<SharedInstanceEvent>,
    ) -> Option<SharedInstanceEvent> {
        timeout(WAIT, events.recv()).await.expect("a state event arrives in time")
    }

    /// A transport-enabled node with a `LocalUnixServer` bound at `socket`; the socket's
    /// appearance on disk is the listening signal.
    #[deny(clippy::await_holding_invalid_type)]
    async fn unix_shared_instance_on(socket: &std::path::Path) -> Transport {
        let mut config = TransportConfig::new(
            "probe-unix-shared-instance",
            &PrivateIdentity::new_from_rand(OsRng),
            false,
        );
        config.set_transport_enabled(true);
        let transport = Transport::new(config);
        let server = LocalUnixServer::new_endpoint(
            LocalUnixEndpoint::filesystem(socket.to_path_buf()),
            transport.iface_manager(),
        );
        transport.iface_manager().lock().await.spawn(server, LocalUnixServer::spawn);
        let deadline = Instant::now() + WAIT;
        while !socket.exists() {
            assert!(Instant::now() < deadline, "timed out waiting for {}", socket.display());
            tokio::time::sleep(POLL).await;
        }
        transport
    }

    #[tokio::test]
    #[deny(clippy::await_holding_invalid_type)]
    async fn first_connect_timeout_over_an_unbound_unix_path_reads_not_found_then_ends() {
        let dir = tempfile::tempdir().expect("socket dir");
        let socket = dir.path().join("never-bound.sock");
        let transport = Transport::new(TransportConfig::new(
            "probe-unix-first-connect-client",
            &PrivateIdentity::new_from_rand(OsRng),
            false,
        ));
        let (events_tx, mut events) = mpsc::channel(64);

        let started = Instant::now();
        let outcome = transport
            .attach_shared_instance(
                SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.clone())),
                SharedInstanceOptions::default()
                    .with_reconnect_wait(RECONNECT_WAIT)
                    .with_state_events(events_tx)
                    .with_first_connect_timeout(FIRST_CONNECT),
            )
            .await;
        let elapsed = started.elapsed();
        assert!(
            matches!(outcome, Err(RnsError::SharedInstanceConnectTimeout)),
            "the unix attach gives up on the first connect: {outcome:?}"
        );
        assert!(
            elapsed >= FIRST_CONNECT && elapsed < Duration::from_secs(5),
            "the attach waited about the timeout: {elapsed:?}"
        );
        assert_eq!(transport.shared_instance(), None, "the slot is released");
        assert!(!transport.connected_to_shared_instance().await, "the posture is off");
        assert!(
            !transport.detach_shared_instance().await,
            "a detach after the timeout finds nothing"
        );
        assert!(!socket.exists(), "the client never created the path it dialed");

        let mut read = Vec::new();
        while let Some(event) = next_event(&mut events).await {
            read.push(event);
        }
        assert_eq!(read.first(), Some(&SharedInstanceEvent::Connecting), "{read:?}");
        assert!(
            read.len() >= 2,
            "a 300 ms wait over 50 ms redials reads at least one failure: {read:?}"
        );
        assert!(
            read[1..].iter().all(|event| *event
                == SharedInstanceEvent::DialFailed { reason: DialFailureKind::NotFound }),
            "only NotFound dials follow Connecting on an unbound unix path: {read:?}"
        );

        // The socket appears: the same transport attaches again, the timeout satisfied.
        let shared = unix_shared_instance_on(&socket).await;
        let (events_tx, mut events) = mpsc::channel(64);
        let iface = transport
            .attach_shared_instance(
                SharedInstanceEndpoint::Unix(LocalUnixEndpoint::filesystem(socket.clone())),
                SharedInstanceOptions::default()
                    .with_reconnect_wait(RECONNECT_WAIT)
                    .with_state_events(events_tx)
                    .with_first_connect_timeout(Duration::from_secs(5)),
            )
            .await
            .expect("a second unix attach succeeds once the owner binds the path");
        assert_eq!(transport.shared_instance(), Some(iface));
        assert!(transport.connected_to_shared_instance().await);
        assert_eq!(next_event(&mut events).await, Some(SharedInstanceEvent::Connecting));
        assert_eq!(next_event(&mut events).await, Some(SharedInstanceEvent::Connected));

        assert!(transport.detach_shared_instance().await);
        timeout(WAIT, transport.stop()).await.expect("client stop completes");
        timeout(WAIT, shared.stop()).await.expect("unix shared instance stop completes");
    }
}
