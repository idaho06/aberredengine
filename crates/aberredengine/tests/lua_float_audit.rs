//! Determinism audit: Lua numbers cross into Rust as `f32`.
//!
//! LuaJIT numbers are `f64`; gameplay fields are `f32`. A Lua number bound for
//! a gameplay field is truncated exactly once, at the boundary -- an `f32`
//! argument of a `register_cmd!`/`register_getter!`/`builder_method!`
//! function, `table.get::<f32>`, or an immediate `n as f32` on a raw
//! `LuaValue::Number`. Rust code never carries the `f64` further, so the result
//! is always `(lua_value) as f32`, whichever path the value took.
//!
//! The rule over `crates/aberred-lua/src/`: outside `#[cfg(test)]` items, no
//! `f64` token, no `f64` accessor or alias (`as_f64`, `as_number`,
//! `LuaNumber`), and no `LuaValue::Number` binding without an `as f32` on the
//! same line, except the lines in [`ALLOWED`]. A token rule rather than a parse
//! of the registration macros -- any `f64` in a boundary declaration is caught
//! regardless of which macro or call shape it sits in.
//! The scan root comes from `CARGO_MANIFEST_DIR`, so the runtime CWD doesn't
//! matter (same shape as `rng_audit.rs`).

use std::path::{Path, PathBuf};

/// Allowed non-test `f64` lines: `(file under aberred-lua/src, line substring, reason)`.
/// Each entry must match exactly one offending line, so a stale entry fails too.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "resources/lua_runtime/entity_builder/behavior.rs",
        "table.get::<Option<f64>>(field)",
        "integer validation: read as f64 to reject fractional values before casting to u32",
    ),
    (
        "resources/lua_runtime/entity_builder/behavior.rs",
        "n <= u32::MAX as f64",
        "integer validation: the u32 range check of the same read",
    ),
    (
        "resources/lua_runtime/entity_builder/behavior.rs",
        "LuaValue::Number(n) => {",
        "particle ttl: the arm's only statement casts `n as f32`",
    ),
];

fn lua_src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/aberredengine should be one level under crates/")
        .join("aberred-lua/src")
}

fn walk_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if entry.file_type().expect("file type").is_dir() {
            walk_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// `(1-based line number, line)` of every non-comment line of `content`
/// that [`carries_f64`], outside a `#[cfg(test)]` item.
///
/// A `#[cfg(test)]` item is skipped up to its matching closing brace, or to its
/// `;` when it has no body. Brace counting and `//` stripping ignore string and
/// char literals, which is enough for this crate's source.
fn f64_lines(content: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let (mut in_test, mut depth, mut opened) = (false, 0i32, false);
    for (i, line) in content.lines().enumerate() {
        let mut code = line.split_once("//").map_or(line, |(code, _)| code).trim();
        if let Some(rest) = code.strip_prefix("#[cfg(test)]").filter(|_| !in_test) {
            (in_test, depth, opened, code) = (true, 0, false, rest.trim());
        }
        if !in_test {
            if carries_f64(code) {
                found.push((i + 1, line));
            }
            continue;
        }
        for ch in code.chars() {
            match ch {
                '{' => (depth, opened) = (depth + 1, true),
                '}' => depth -= 1,
                _ => {}
            }
        }
        in_test = if opened {
            depth > 0
        } else {
            !code.ends_with(';')
        };
    }
    found
}

/// Whether `code` names an `f64`: the type itself, an `f64` accessor or alias,
/// or a `LuaValue::Number` binding not truncated with `as f32` on the same line.
fn carries_f64(code: &str) -> bool {
    ["f64", "as_f64", "as_number", "LuaNumber"]
        .iter()
        .any(|token| has_token(code, token))
        || (has_token(code, "Number") && !code.contains("as f32"))
}

/// Whether `code` holds `token` as a whole identifier (not inside `f64x`, ...).
fn has_token(code: &str, token: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    code.match_indices(token).any(|(at, _)| {
        !code[..at].chars().next_back().is_some_and(is_ident)
            && !code[at + token.len()..]
                .chars()
                .next()
                .is_some_and(is_ident)
    })
}

#[test]
fn scanner_flags_an_f64_token() {
    let src = "register_cmd!(engine, \"set_x\", |x| f64, Cmd::SetX(x));\n";
    assert_eq!(f64_lines(src).len(), 1);
}

#[test]
fn scanner_ignores_comments_identifiers_and_cfg_test_items() {
    let src = "\
// a comment naming f64
/// a doc comment naming f64
let x: f32 = t.get(\"x\")?; // trailing f64 note
fn as_f64_like() {}
#[cfg(test)]
mod tests {
    fn helper() -> f64 {
        1.0
    }
}
#[cfg(test)]
use std::f64::consts::PI;
fn after() -> f64 { 0.0 }
";
    let lines: Vec<usize> = f64_lines(src).into_iter().map(|(n, _)| n).collect();
    assert_eq!(lines, [13], "only `after`, past both #[cfg(test)] items");
}

#[test]
fn scanner_flags_a_lua_number_unless_it_truncates_on_the_spot() {
    let src = "\
LuaValue::Number(n) => Ok(UniformValue::Float(n as f32)),
LuaValue::Number(n) => carry(n),
let n = v.as_f64();
let n: LuaNumber = t.get(1)?;
";
    let lines: Vec<usize> = f64_lines(src).into_iter().map(|(n, _)| n).collect();
    assert_eq!(lines, [2, 3, 4]);
}

#[test]
fn lua_numbers_cross_into_rust_as_f32() {
    let root = lua_src_dir();
    let mut paths = Vec::new();
    walk_rs_files(&root, &mut paths);
    paths.sort();

    let mut matched = vec![0usize; ALLOWED.len()];
    let mut offenders = Vec::new();
    for path in &paths {
        let rel = path
            .strip_prefix(&root)
            .expect("under aberred-lua/src")
            .to_string_lossy()
            .into_owned();
        let content = std::fs::read_to_string(path).expect("read source file");
        for (n, line) in f64_lines(&content) {
            match ALLOWED
                .iter()
                .position(|(file, needle, _)| *file == rel && line.contains(needle))
            {
                Some(i) => matched[i] += 1,
                None => offenders.push(format!("{rel}:{n}: {}", line.trim())),
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "found f64 in aberred-lua outside test code -- declare Lua-facing numeric \
         arguments as f32 so mlua truncates at the boundary (or add an ALLOWED \
         entry with a reason if the destination isn't an f32 field):\n{}",
        offenders.join("\n")
    );
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&matched)
        .filter(|(_, count)| **count != 1)
        .map(|((file, needle, reason), count)| {
            format!("{file}: `{needle}` ({reason}) matched {count} lines")
        })
        .collect();
    assert!(
        stale.is_empty(),
        "ALLOWED entries must each match exactly one line:\n{}",
        stale.join("\n")
    );
}
