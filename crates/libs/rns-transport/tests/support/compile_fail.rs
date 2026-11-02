//! The `rustc` harness the compile-fail tests drive against the `librns_transport` rlib this
//! test binary linked: a scratch directory, the deps directory cargo linked from, the rlib
//! candidates in it, and one `rustc` invocation per snippet.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Scratch directory removed when the test ends, panic included.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rns-transport-{label}-{}-{}",
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
pub fn deps_dir() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary should know its own path");
    exe.parent().map(Path::to_path_buf).expect("test binary should live in target/<profile>/deps")
}

/// Every `librns_transport-*.rlib` in the deps directory, newest first. More than one can
/// exist (different feature sets or an earlier build); the harness picks the one the
/// migrated consumer links against.
pub fn candidate_rlibs(deps: &Path) -> Vec<PathBuf> {
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

pub fn rustc(scratch: &Path, deps: &Path, rlib: &Path, name: &str, source: &str) -> Output {
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
