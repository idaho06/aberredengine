//! Determinism audit: no stray RNG usage outside `SimRng`
//! (determinism-03-seeded-rng.md).
//!
//! Mirrors phase 02's `ambiguity_audit` precedent (`src/engine_app/schedule.rs`)
//! -- a real, in-code test rather than a doc checklist. Two rules over `src/`:
//!
//! 1. No `Local<...Rng>` anywhere -- every sim-schedule system must draw from
//!    the shared `ResMut<SimRng>` (or `GameCtx::sim_rng`), never a private
//!    per-system stream (which would reintroduce the exact nondeterminism
//!    this phase removes).
//! 2. `fastrand::Rng::new()` (the entropy-seeding constructor) is confined to
//!    its one legitimate call site, `setup_logic_world`'s non-deterministic-
//!    mode branch (`src/engine_app/logic_world.rs`) -- everywhere else,
//!    including test-world helpers seeding a placeholder `SimRng`, must go
//!    through `SimRng::from_seed(seed)` so the seed is always a concrete,
//!    known value (see `SimRng`'s doc comment).
//!
//! No `rand::` crate exists in this workspace (`fastrand` is the only RNG
//! dependency), so there is nothing to grep for there today; if one is ever
//! added, extend this audit rather than assuming it stays out.

use std::path::{Path, PathBuf};

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

/// Every non-comment line under `src/` (skipping `excluded_file`, if given)
/// matching `predicate`, formatted as `path:line: text` for an assertion
/// message.
fn scan_src(excluded_file: Option<&Path>, predicate: impl Fn(&str) -> bool) -> Vec<String> {
    let mut files = Vec::new();
    walk_rs_files(Path::new("src"), &mut files);

    let mut offenders = Vec::new();
    for path in &files {
        if Some(path.as_path()) == excluded_file {
            continue;
        }
        let content = std::fs::read_to_string(path).expect("read source file");
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
    let offenders = scan_src(Some(Path::new("src/engine_app/logic_world.rs")), |line| {
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
