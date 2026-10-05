//! Drift test for `RUST-GAME-GUIDE.md`.
//!
//! Every fence opened with exactly ```` ```rust ```` must appear, line for line,
//! in some `guide-check/src/*.rs` file, which `just check-guide` compiles as a
//! downstream game would. Lines are compared trimmed with blank lines dropped,
//! so a snippet may sit indented inside a module and be surrounded by glue.
//! ```` ```rust,ignore ```` fences (illustrative fragments) are skipped.
//!
//! Every in-page link `](#anchor)` must also match the GitHub slug of some
//! heading, so moving or renaming a section can't silently break a link.

use std::collections::HashMap;
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

/// One markdown line, classified by fence state.
enum MdLine<'a> {
    /// Outside any fence.
    Text(&'a str),
    /// A fence's opening line, trimmed and unquoted (e.g. ```` ```rust ````).
    Open(&'a str),
    /// A line inside a fence, with one `>` level stripped for a quoted fence.
    Body(&'a str),
    /// A fence's closing line.
    Close,
}

/// Classifies every line of `markdown`, in order. Every fence is tracked,
/// whatever its info string (`rust,ignore`, `toml`, bare, ...), so its closing
/// line isn't taken for an opening one.
fn classify_lines(markdown: &str) -> impl Iterator<Item = MdLine<'_>> {
    // `Some(quoted)` while inside a fence.
    let mut fence: Option<bool> = None;
    markdown.lines().map(move |raw| match fence {
        Some(quoted) => {
            let line = if quoted {
                unquote(raw).unwrap_or(raw)
            } else {
                raw
            };
            if line.trim().starts_with("```") {
                fence = None;
                MdLine::Close
            } else {
                MdLine::Body(line)
            }
        }
        None => {
            let (quoted, opener) = match unquote(raw) {
                Some(inner) => (true, inner.trim()),
                None => (false, raw.trim()),
            };
            if opener.starts_with("```") {
                fence = Some(quoted);
                MdLine::Open(opener)
            } else {
                MdLine::Text(raw)
            }
        }
    })
}

fn extract_rust_fences(markdown: &str) -> Vec<Fence> {
    let mut fences = Vec::new();
    // 1-based opening line and body of the open plain-`rust` fence, if any.
    let mut current: Option<(usize, Vec<&str>)> = None;
    for (idx, line) in classify_lines(markdown).enumerate() {
        match line {
            MdLine::Open("```rust") => current = Some((idx + 1, Vec::new())),
            MdLine::Body(body_line) => {
                if let Some((_, body)) = current.as_mut() {
                    body.push(body_line);
                }
            }
            MdLine::Close => {
                if let Some((line, body)) = current.take() {
                    fences.push(Fence {
                        line,
                        lines: normalize(body),
                    });
                }
            }
            MdLine::Open(_) | MdLine::Text(_) => {}
        }
    }
    fences
}

/// Lines outside every fence.
fn text_lines(markdown: &str) -> impl Iterator<Item = &str> {
    classify_lines(markdown).filter_map(|line| match line {
        MdLine::Text(text) => Some(text),
        _ => None,
    })
}

/// GitHub's anchor slug for every ATX heading outside a fence, in document
/// order. The heading text is lowercased; every character that isn't
/// alphanumeric (Unicode included), a space, `-` or `_` is dropped (backticks,
/// parentheses, `.`, `—`, ...: `6.1` → `61`); then each space becomes `-` on
/// its own (`A — B` → `a--b`). A repeated slug gets `-1`, `-2`, ... appended,
/// in document order.
fn heading_slugs(markdown: &str) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    text_lines(markdown)
        .filter_map(|text| {
            let level = text.len() - text.trim_start_matches('#').len();
            if !(1..=6).contains(&level) {
                return None;
            }
            let title = text[level..].strip_prefix(' ')?;
            let slug: String = title
                .trim()
                .to_lowercase()
                .chars()
                .filter_map(|c| match c {
                    ' ' => Some('-'),
                    '-' | '_' => Some(c),
                    c if c.is_alphanumeric() => Some(c),
                    _ => None,
                })
                .collect();
            let count = seen.entry(slug.clone()).or_insert(0);
            let unique = match *count {
                0 => slug,
                n => format!("{slug}-{n}"),
            };
            *count += 1;
            Some(unique)
        })
        .collect()
}

/// The `anchor` of every `](#anchor)` link outside a fence, in order.
fn link_anchors(markdown: &str) -> Vec<&str> {
    text_lines(markdown)
        .flat_map(|text| {
            text.split("](#")
                .skip(1)
                .filter_map(|rest| rest.split_once(')').map(|(anchor, _)| anchor))
        })
        .collect()
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

fn read_guide() -> String {
    let guide_path = workspace_root().join("RUST-GAME-GUIDE.md");
    fs::read_to_string(&guide_path).unwrap_or_else(|e| panic!("{}: {e}", guide_path.display()))
}

#[test]
fn every_guide_rust_fence_is_compiled_in_guide_check() {
    let guide = read_guide();
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
fn guide_internal_links_resolve() {
    let guide = read_guide();
    let slugs = heading_slugs(&guide);
    let broken: Vec<&str> = link_anchors(&guide)
        .into_iter()
        .filter(|anchor| !slugs.iter().any(|slug| slug == anchor))
        .collect();
    assert!(
        broken.is_empty(),
        "RUST-GAME-GUIDE.md links to anchors that no heading produces: {broken:?}"
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

#[test]
fn heading_slugs_follow_github_rules() {
    let md = "# Title\n## 6.1 What `ctx` does (today)\n### Approach A — SceneManager\n\
              ```rust\n#[derive(Component)]\n```\n> ```\n> # Quoted fence\n> ```\n\
              #### Dup\n#### Dup\n#### Dup\n#not-a-heading\n";
    assert_eq!(
        heading_slugs(md),
        vec![
            "title",
            "61-what-ctx-does-today",
            "approach-a--scenemanager",
            "dup",
            "dup-1",
            "dup-2",
        ]
    );
}

#[test]
fn link_anchors_skip_fenced_lines() {
    let md = "see [a](#one) and [b](#two-2)\n```\n[c](#fenced)\n```\n[d](https://x.y/#ext)\n";
    assert_eq!(link_anchors(md), vec!["one", "two-2"]);
}
