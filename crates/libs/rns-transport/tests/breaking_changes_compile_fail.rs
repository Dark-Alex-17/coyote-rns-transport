//! The `### Breaking` migrations in `CHANGELOG.md` promise what no longer compiles outside
//! the crate: a struct literal of `AnnounceEvent` (`#[non_exhaustive]`), a struct literal of
//! `DeliveryReceipt` (private `link_id`), `ResourceEventKind::OutboundRejected` written as a
//! unit pattern, and a struct literal of any fork-new public struct (every field private).
//! Those are compile-time facts an in-crate `#[test]` cannot observe, so this file drives
//! `rustc` the way cargo does, against the `librns_transport` rlib this very test binary
//! linked, and checks that each rejected snippet fails with the promised diagnostic while
//! the migrated counterpart compiles. No extra dependency, no nested cargo, no shared lock.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The migrated shapes the CHANGELOG tells a consumer to write. Compiling this first proves
/// the harness itself works against the chosen rlib, so a rejected snippet below fails for
/// the promised reason and not because the harness is broken.
const MIGRATED_CONSUMER: &str = r#"
use rns_transport::destination::link::LinkEvent;
use rns_transport::destination::request::{PathHash, RequestOptions};
use rns_transport::error::RnsError;
use rns_transport::hash::Hash;
use rns_transport::resource::ResourceEventKind;
use rns_transport::transport::{AnnounceEvent, DeliveryReceipt};

pub fn announce_is_path_response(event: &AnnounceEvent) -> bool {
    let AnnounceEvent { is_path_response, .. } = event;
    *is_path_response
}

pub fn receipt_matches(message_id: [u8; 32], hash: &Hash) -> bool {
    DeliveryReceipt::new(message_id).matches_packet_hash(hash)
}

pub fn was_rejected(kind: &ResourceEventKind) -> bool {
    matches!(kind, ResourceEventKind::OutboundRejected { .. })
}

pub fn proven(event: &LinkEvent) -> bool {
    match event {
        LinkEvent::PacketProven { .. } => true,
        _ => false,
    }
}

pub fn no_path(error: &RnsError) -> bool {
    match error {
        RnsError::NoPath => true,
        _ => false,
    }
}

pub fn options() -> RequestOptions {
    RequestOptions::default()
}

pub fn path_hash(path: &str) -> PathHash {
    PathHash::of(path)
}
"#;

/// Each rejected snippet with the diagnostic fragment the migration promises.
const REJECTED: &[(&str, &str, &str)] = &[
    (
        "announce_event_struct_literal",
        "use rns_transport::transport::AnnounceEvent;\n\
         pub fn build() -> AnnounceEvent { AnnounceEvent { is_path_response: true } }\n",
        "cannot create non-exhaustive struct using struct expression",
    ),
    (
        "delivery_receipt_struct_literal",
        "use rns_transport::hash::Hash;\n\
         use rns_transport::transport::DeliveryReceipt;\n\
         pub fn build(hash: Hash) -> DeliveryReceipt { DeliveryReceipt { message_id: hash } }\n",
        "cannot construct `DeliveryReceipt` with struct literal syntax due to private fields",
    ),
    (
        "outbound_rejected_unit_pattern",
        "use rns_transport::resource::ResourceEventKind;\n\
         pub fn was_rejected(kind: &ResourceEventKind) -> bool {\n\
             match kind { ResourceEventKind::OutboundRejected => true, _ => false }\n\
         }\n",
        "found struct variant `ResourceEventKind::OutboundRejected`",
    ),
    (
        "request_options_struct_literal",
        "use rns_transport::destination::request::RequestOptions;\n\
         pub fn build() -> RequestOptions { RequestOptions {} }\n",
        "cannot construct `RequestOptions` with struct literal syntax due to private fields",
    ),
    (
        "request_receipt_struct_literal",
        "use rns_transport::destination::request::RequestReceipt;\n\
         pub fn build() -> RequestReceipt { RequestReceipt {} }\n",
        "cannot construct `RequestReceipt` with struct literal syntax due to private fields",
    ),
    (
        "request_frame_struct_literal",
        "use rns_transport::destination::request::RequestFrame;\n\
         pub fn build() -> RequestFrame { RequestFrame {} }\n",
        "cannot construct `RequestFrame` with struct literal syntax due to private fields",
    ),
    (
        "path_hash_tuple_literal",
        "use rns_transport::destination::request::PathHash;\n\
         pub fn build() -> PathHash { PathHash([0u8; 16]) }\n",
        "cannot initialize a tuple struct which contains private fields",
    ),
];

/// Scratch directory removed when the test ends, panic included.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rns-transport-breaking-compile-fail-{}-{}",
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

/// The `deps` directory this test binary was linked from; cargo puts the rlib of every
/// dependency, `rns_transport` included, next to the test executable.
fn deps_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary should know its own path");
    exe.parent().map(Path::to_path_buf).expect("test binary should live in target/<profile>/deps")
}

/// Every `librns_transport-*.rlib` in the deps directory, newest first. More than one can
/// exist (different feature sets or an earlier build); the harness picks the one the
/// migrated consumer links against.
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

/// The rlib the migrated consumer compiles against, with the deps directory. Fails loudly,
/// with every candidate's diagnostics, when no rlib works: that is a harness problem, not a
/// verdict on the crate.
fn working_rlib(scratch: &Path) -> (PathBuf, PathBuf) {
    let deps = deps_dir();
    let candidates = candidate_rlibs(&deps);
    assert!(!candidates.is_empty(), "no librns_transport-*.rlib next to {}", deps.display());
    let mut failures = Vec::new();
    for rlib in candidates {
        let output = rustc(scratch, &deps, &rlib, "migrated_consumer", MIGRATED_CONSUMER);
        if output.status.success() {
            return (deps, rlib);
        }
        failures.push(format!("{}:\n{}", rlib.display(), String::from_utf8_lossy(&output.stderr)));
    }
    panic!(
        "the migrated consumer from the CHANGELOG should compile against the crate's rlib; \
         every candidate failed:\n{}",
        failures.join("\n")
    );
}

#[test]
fn changelog_breaking_migrations_compile_and_the_shapes_they_replace_do_not() {
    let scratch = Scratch::new();
    let (deps, rlib) = working_rlib(&scratch.0);
    let mut unexpected = Vec::new();
    for (name, source, promised) in REJECTED {
        let output = rustc(&scratch.0, &deps, &rlib, name, source);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            unexpected.push(format!("{name}: compiled, but the CHANGELOG says it no longer does"));
        } else if !stderr.contains(promised) {
            unexpected.push(format!(
                "{name}: failed, but not with the promised diagnostic {promised:?}:\n{stderr}"
            ));
        }
    }
    assert!(unexpected.is_empty(), "{}", unexpected.join("\n\n"));
}
