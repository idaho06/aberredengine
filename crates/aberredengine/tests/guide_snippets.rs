//! Drift test for `RUST-GAME-GUIDE.md`.
//!
//! Every fence opened with exactly ```` ```rust ```` must appear, line for line,
//! in some `guide-check/src/*.rs` file, which `just check-guide` compiles as a
//! downstream game would. Lines are compared trimmed with blank lines dropped,
//! so a snippet may sit indented inside a module and be surrounded by glue.
//! ```` ```rust,ignore ```` fences (illustrative fragments) are skipped.

use std::fs;
use std::path::{Path, PathBuf};

struct Fence {
    /// 1-based line number of the opening ```` ```rust ```` line.
    line: usize,
    lines: Vec<String>,
}

fn normalize<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    lines
        .into_iter()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Strips one blockquote level (`>`) from a line, if present.
fn unquote(line: &str) -> Option<&str> {
    line.trim_start().strip_prefix('>')
}

fn extract_rust_fences(markdown: &str) -> Vec<Fence> {
    struct Open<'a> {
        /// 0 for a fence that isn't plain `rust`: its body is skipped.
        line: usize,
        quoted: bool,
        body: Vec<&'a str>,
    }
    let mut fences = Vec::new();
    let mut current: Option<Open> = None;
    for (idx, raw) in markdown.lines().enumerate() {
        match current.as_mut() {
            Some(open) => {
                let line = if open.quoted {
                    unquote(raw).unwrap_or(raw)
                } else {
                    raw
                };
                if line.trim().starts_with("```") {
                    if open.line != 0 {
                        fences.push(Fence {
                            line: open.line,
                            lines: normalize(open.body.iter().copied()),
                        });
                    }
                    current = None;
                } else {
                    open.body.push(line);
                }
            }
            None => {
                let (quoted, opener) = match unquote(raw) {
                    Some(inner) => (true, inner.trim()),
                    None => (false, raw.trim()),
                };
                if opener.starts_with("```") {
                    // Any fence other than plain `rust` (`rust,ignore`, `toml`, bare, ...)
                    // is still tracked, so its closing line isn't taken for an opening one.
                    current = Some(Open {
                        line: if opener == "```rust" { idx + 1 } else { 0 },
                        quoted,
                        body: Vec::new(),
                    });
                }
            }
        }
    }
    fences
}

fn contains_contiguous(haystack: &[String], needle: &[String]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Normalized lines of every `guide-check/src/*.rs` file; empty if the
/// directory is missing, so every fence reports as uncovered.
fn example_sources() -> Vec<Vec<String>> {
    let dir = workspace_root().join("guide-check/src");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|p| {
            let text = fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            normalize(text.lines())
        })
        .collect()
}

#[test]
fn every_guide_rust_fence_is_compiled_in_guide_check() {
    let guide_path = workspace_root().join("RUST-GAME-GUIDE.md");
    let guide =
        fs::read_to_string(&guide_path).unwrap_or_else(|e| panic!("{}: {e}", guide_path.display()));
    let fences = extract_rust_fences(&guide);
    assert!(!fences.is_empty(), "no ```rust fences found in the guide");

    let sources = example_sources();
    let uncovered: Vec<String> = fences
        .iter()
        .filter(|f| !sources.iter().any(|s| contains_contiguous(s, &f.lines)))
        .map(|f| {
            format!(
                "  guide line {}: {}",
                f.line,
                f.lines.first().map_or("<empty>", String::as_str)
            )
        })
        .collect();

    assert!(
        uncovered.is_empty(),
        "{} of {} ```rust fences in RUST-GAME-GUIDE.md are not found verbatim in \
         guide-check/src/*.rs (copy the fence there, or mark an illustrative \
         fragment ```rust,ignore):\n{}",
        uncovered.len(),
        fences.len(),
        uncovered.join("\n")
    );
}

#[test]
fn extraction_takes_only_plain_rust_fences() {
    let md = "intro\n```rust\nfn a() {}\n\n    let x = 1;\n```\n```rust,ignore\nfn b() {}\n```\n\
              ```toml\n[x]\n```\n```\nplain\n```\n  ```rust\n  fn c() {}\n  ```\n\
              > ```rust\n> fn d() {\n>\n>     >\n> }\n> ```\n";
    let fences = extract_rust_fences(md);
    let got: Vec<(usize, Vec<String>)> = fences.into_iter().map(|f| (f.line, f.lines)).collect();
    assert_eq!(
        got,
        vec![
            (2, vec!["fn a() {}".to_owned(), "let x = 1;".to_owned()]),
            (16, vec!["fn c() {}".to_owned()]),
            (
                19,
                vec!["fn d() {".to_owned(), ">".to_owned(), "}".to_owned()]
            ),
        ]
    );
}

#[test]
fn containment_ignores_indentation_and_blank_lines_but_not_gaps() {
    let source =
        normalize("mod m {\n    fn a() {\n\n        b();\n    }\n    // glue\n}\n".lines());
    assert!(contains_contiguous(
        &source,
        &normalize(["fn a() {", "b();", "}"])
    ));
    assert!(!contains_contiguous(&source, &normalize(["fn a() {", "}"])));
    assert!(!contains_contiguous(&source, &normalize(["fn z() {}"])));
}
