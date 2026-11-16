//! The `rustc` harness the compile-fail tests drive against the `librns_transport` rlib this
//! test binary linked: a scratch directory, the artifact root cargo linked from, the rlib
//! candidates under it, and one `rustc` invocation per snippet.

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

/// The `target/<profile>` directory this test binary was built into, the root under which
/// cargo keeps every dependency's rlib. Older cargos put them all in `<root>/deps` and run the
/// test executable from there; cargo 1.100 keeps each unit's output under
/// `<root>/build/<crate>/<hash>/out` and runs the executable from its own such directory. The
/// root is the nearest ancestor of the executable that owns a `build` or `deps` directory.
pub fn artifact_root() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary should know its own path");
    exe.ancestors()
        .find(|dir| dir.join("build").is_dir() || dir.join("deps").is_dir())
        .map(Path::to_path_buf)
        .expect("test binary should live under target/<profile>")
}

/// Every directory under the root that holds a dependency artifact (an rlib, an rmeta, or a
/// proc-macro dylib), in the order rustc should search them: `<root>/deps` when it exists,
/// else each `<root>/build/<crate>/<hash>/out` with such a file in it.
pub fn link_search_dirs(root: &Path) -> Vec<PathBuf> {
    let deps = root.join("deps");
    if deps.is_dir() {
        return vec![deps];
    }
    let mut dirs = Vec::new();
    for crate_dir in read_dirs(&root.join("build")) {
        for unit_dir in read_dirs(&crate_dir) {
            let out = unit_dir.join("out");
            if read_entries(&out).any(|path| is_dependency_artifact(&path)) {
                dirs.push(out);
            }
        }
    }
    dirs
}

fn is_dependency_artifact(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ext == "rlib" || ext == "rmeta" || ext == "dylib" || ext == "so" || ext == "dll"
    })
}

/// Every `librns_transport-*.rlib` under the root, newest first. More than one can exist
/// (different feature sets or an earlier build); the harness picks the one the migrated
/// consumer links against.
pub fn candidate_rlibs(root: &Path) -> Vec<PathBuf> {
    let mut found: Vec<(std::time::SystemTime, PathBuf)> = link_search_dirs(root)
        .iter()
        .flat_map(|dir| read_entries(dir))
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

pub fn rustc(scratch: &Path, root: &Path, rlib: &Path, name: &str, source: &str) -> Output {
    let file = scratch.join(format!("{name}.rs"));
    fs::write(&file, source).expect("snippet source should be writable");
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut command = Command::new(compiler);
    command
        .arg("--edition")
        .arg("2021")
        .arg("--crate-type")
        .arg("lib")
        .arg("--crate-name")
        .arg(name);
    for dir in link_search_dirs(root) {
        command.arg("-L").arg(format!("dependency={}", dir.display()));
    }
    command
        .arg("--extern")
        .arg(format!("rns_transport={}", rlib.display()))
        .arg("--out-dir")
        .arg(scratch)
        .arg(&file)
        .output()
        .expect("rustc should be runnable from a cargo test")
}

fn read_entries(dir: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|entry| entry.path())
}

fn read_dirs(dir: &Path) -> Vec<PathBuf> {
    read_entries(dir).filter(|path| path.is_dir()).collect()
}
