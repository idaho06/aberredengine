//! Determinism audit: no stray RNG usage outside `SimRng`
//! (determinism-03-seeded-rng.md).
//!
//! Mirrors phase 02's `ambiguity_audit` precedent (`src/engine_app/schedule.rs`)
//! -- a real, in-code test rather than a doc checklist. Two rules over every
//! workspace member's `src/` tree:
//!
//! 1. No `Local<...Rng>` anywhere -- every sim-schedule system must draw from
//!    the shared `ResMut<SimRng>` (or `GameCtx::sim_rng`), never a private
//!    per-system stream (which would reintroduce the exact nondeterminism
//!    this phase removes).
//! 2. `fastrand::Rng::new()` (the entropy-seeding constructor) is confined to
//!    its one legitimate call site, `setup_logic_world`'s non-deterministic-
//!    mode branch (`crates/aberredengine/src/engine_app/logic_world.rs`) --
//!    everywhere else, including test-world helpers seeding a placeholder
//!    `SimRng`, must go through `SimRng::from_seed(seed)` so the seed is
//!    always a concrete, known value (see `SimRng`'s doc comment).
//!
//! No `rand::` crate exists in this workspace (`fastrand` is the only RNG
//! dependency), so there is nothing to grep for there today; if one is ever
//! added, extend this audit rather than assuming it stays out.
//!
//! Scans all five workspace member crates' `src/` trees, not just this
//! crate's own -- most `SimRng` usage actually lives in `aberred-core`
//! (gameplay systems) and `aberred-lua`, not the facade. The scan root is
//! computed from `CARGO_MANIFEST_DIR` (a compile-time absolute path to this
//! crate's own manifest directory) rather than a CWD-relative `"src"`, so
//! this audit is correct regardless of the test binary's runtime CWD --
//! unlike `assets`/`config.ini`, which needed relative symlinks to cope with
//! `cargo test` running with CWD set to the package root (see Phase 1's
//! writeup in `docs/plans/workspaces-implementation.md`), this file sidesteps
//! that pitfall entirely by never depending on CWD in the first place.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Absolute path to the workspace's `crates/` directory, computed at compile
/// time from this crate's own manifest directory (`crates/aberredengine`, one
/// level under it).
fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/aberredengine should be one level under crates/")
        .to_path_buf()
}

/// Every member crate's directory name, relative to `crates_dir()`. Mirrors
/// the root `Cargo.toml`'s `[workspace] members` list -- update both
/// together when a crate is added to or removed from the workspace.
const MEMBER_CRATES: &[&str] = &[
    "aberred-core",
    "aberred-render",
    "aberred-audio",
    "aberred-lua",
    "aberredengine",
];

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `.rs` file under any member crate's `src/`, with its contents,
/// read once and cached -- both `#[test]` fns below scan this same set (Rust
/// runs `#[test]` fns concurrently by default, and each would otherwise
/// re-walk and re-read all five crates' source trees independently).
fn source_files() -> &'static [(PathBuf, String)] {
    static FILES: OnceLock<Vec<(PathBuf, String)>> = OnceLock::new();
    FILES.get_or_init(|| {
        let crates_dir = crates_dir();
        let mut paths = Vec::new();
        for crate_name in MEMBER_CRATES {
            walk_rs_files(&crates_dir.join(crate_name).join("src"), &mut paths);
        }
        paths
            .into_iter()
            .map(|path| {
                let content = std::fs::read_to_string(&path).expect("read source file");
                (path, content)
            })
            .collect()
    })
}

/// Every non-comment line under any member crate's `src/` (skipping
/// `excluded_file`, if given) matching `predicate`, formatted as
/// `path:line: text` for an assertion message.
fn scan_src(excluded_file: Option<&Path>, predicate: impl Fn(&str) -> bool) -> Vec<String> {
    let mut offenders = Vec::new();
    for (path, content) in source_files() {
        if Some(path.as_path()) == excluded_file {
            continue;
        }
        for (i, line) in content.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            if predicate(line) {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    offenders
}

#[test]
fn no_local_rng_anywhere_in_src() {
    let offenders = scan_src(None, |line| line.contains("Local<") && line.contains("Rng"));

    assert!(
        offenders.is_empty(),
        "found Local<...Rng> usage outside SimRng -- sim systems must draw \
         from the shared ResMut<SimRng>, never a private per-system stream:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn entropy_seeding_confined_to_setup_logic_world() {
    let excluded = crates_dir().join("aberredengine/src/engine_app/logic_world.rs");
    let offenders = scan_src(Some(&excluded), |line| {
        line.contains("fastrand::Rng::new()") || line.contains("Rng::new()")
    });

    assert!(
        offenders.is_empty(),
        "found fastrand::Rng::new() (entropy-seeding) outside setup_logic_world -- \
         every other SimRng construction must go through SimRng::from_seed(seed) \
         so there's a concrete, known seed:\n{}",
        offenders.join("\n")
    );
}
