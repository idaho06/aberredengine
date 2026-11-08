//! Signal value to text lookup shared by signal bindings and dialogue placeholders.
//!
//! `signal_as_str` reads one signal key from any signal source (`WorldSignals`,
//! `SignalSnapshot`, or an entity's `Signals` component) and renders the value as
//! text: integers and scalars through `Display`, strings borrowed as-is, and set
//! flags as `"true"`.
//!
//! The module also parses placeholder templates (`{key}`, `{self.key}`, `{key:.2}`):
//! `validate_signal_text` checks one, and `SignalTextError` says what is wrong with it.

use arrayvec::ArrayString;
use std::fmt::Write as _;

use crate::components::signals::Signals;
use crate::resources::world_signals::SignalsRead;

/// Capacity for a formatted number. `f32`'s `Display` never uses exponent notation, so its
/// widest output is `-f32::MAX` at 40 chars (`i32::MIN` is 11). A buffer that is too small
/// makes `write!` fail part-way and the text shows a truncated number.
pub(crate) const NUM_BUF_LEN: usize = 48;

/// Stack-allocated string for signal-to-text conversion.
/// Uses a [`NUM_BUF_LEN`]-byte ArrayString for numeric types (i32 / f32), borrowed &str for others.
pub(crate) enum SignalStr<'a> {
    Stack(ArrayString<NUM_BUF_LEN>),
    Borrowed(&'a str),
}

impl<'a> SignalStr<'a> {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            SignalStr::Stack(s) => s.as_str(),
            SignalStr::Borrowed(s) => s,
        }
    }
}

/// Read access to the four signal kinds, provided by every signal source:
/// [`WorldSignals`](crate::resources::world_signals::WorldSignals) and
/// [`SignalSnapshot`](crate::resources::world_signals::SignalSnapshot) through
/// [`SignalsRead`], plus the entity [`Signals`] component.
pub(crate) trait SignalLookup {
    fn get_integer(&self, key: &str) -> Option<i32>;
    fn get_scalar(&self, key: &str) -> Option<f32>;
    fn get_string(&self, key: &str) -> Option<&str>;
    fn has_flag(&self, key: &str) -> bool;
}

impl<T: SignalsRead + ?Sized> SignalLookup for T {
    fn get_integer(&self, key: &str) -> Option<i32> {
        SignalsRead::get_integer(self, key)
    }
    fn get_scalar(&self, key: &str) -> Option<f32> {
        SignalsRead::get_scalar(self, key)
    }
    fn get_string(&self, key: &str) -> Option<&str> {
        SignalsRead::get_string(self, key)
    }
    fn has_flag(&self, key: &str) -> bool {
        SignalsRead::has_flag(self, key)
    }
}

impl SignalLookup for Signals {
    fn get_integer(&self, key: &str) -> Option<i32> {
        Signals::get_integer(self, key)
    }
    fn get_scalar(&self, key: &str) -> Option<f32> {
        Signals::get_scalar(self, key)
    }
    fn get_string(&self, key: &str) -> Option<&str> {
        Signals::get_string(self, key)
    }
    fn has_flag(&self, key: &str) -> bool {
        Signals::has_flag(self, key)
    }
}

/// Formats a number with `Display` into a stack buffer.
fn display_to_stack<'a>(v: impl std::fmt::Display) -> SignalStr<'a> {
    let mut buf = ArrayString::<NUM_BUF_LEN>::new();
    let _ = write!(buf, "{}", v);
    SignalStr::Stack(buf)
}

/// Converts a signal value to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag.
/// Returns `None` if the signal key is not found.
pub(crate) fn signal_as_str<'a, S: SignalLookup + ?Sized>(
    src: &'a S,
    key: &str,
) -> Option<SignalStr<'a>> {
    if let Some(v) = src.get_integer(key) {
        return Some(display_to_stack(v));
    }
    if let Some(v) = src.get_scalar(key) {
        return Some(display_to_stack(v));
    }
    if let Some(s) = src.get_string(key) {
        return Some(SignalStr::Borrowed(s));
    }
    if src.has_flag(key) {
        return Some(SignalStr::Borrowed("true"));
    }
    None
}

/// Why a placeholder template is malformed. Every `at` is a byte offset into the template.
///
/// `UnclosedBrace`, `EmptyKey` and `BadPrecision` point at the `{` that opens the placeholder;
/// `StrayCloseBrace` points at the lone `}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignalTextError {
    /// A `{` with no matching `}` (write `{{` for a literal brace).
    #[error("unclosed `{{` at byte {at}; write `{{{{` for a literal brace")]
    UnclosedBrace {
        /// Byte offset of the `{`.
        at: usize,
    },
    /// A `}` that closes nothing (write `}}` for a literal brace).
    #[error("stray `}}` at byte {at}; write `}}}}` for a literal brace")]
    StrayCloseBrace {
        /// Byte offset of the `}`.
        at: usize,
    },
    /// A placeholder with no key, such as `{}` or `{self.}`.
    #[error("placeholder at byte {at} has an empty key")]
    EmptyKey {
        /// Byte offset of the `{`.
        at: usize,
    },
    /// A `:.` suffix that is not `:.0` through `:.6`.
    #[error("placeholder at byte {at} has a bad precision; use `:.0` through `:.6`")]
    BadPrecision {
        /// Byte offset of the `{`.
        at: usize,
    },
}

/// Which signal store a placeholder reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaceholderSource {
    /// `{key}`: the world signals.
    World,
    /// `{self.key}`: the entity `Signals` the caller passes in.
    Entity,
}

/// One parsed `{...}` placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placeholder<'a> {
    pub(crate) source: PlaceholderSource,
    pub(crate) key: &'a str,
    /// Digits after the point from a `:.N` spec, `0..=6`.
    pub(crate) precision: Option<u8>,
}

/// A piece of a template: text to copy, or a placeholder to fill in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Segment<'a> {
    /// Text copied as-is. A `{{` or `}}` escape is the one-brace literal.
    Literal(&'a str),
    Placeholder(Placeholder<'a>),
}

/// Walks a template as [`Segment`]s without allocating.
///
/// A malformed part yields one `Err`, then parsing resumes after it: past the offending
/// brace, or at the `{` that interrupted an unclosed placeholder.
pub(crate) struct Segments<'a> {
    template: &'a str,
    pos: usize,
}

/// Splits `template` into segments.
///
/// Grammar: `{{` and `}}` are literal braces; `{key}` reads a world signal, `{self.key}` an
/// entity signal, and either takes a `:.N` precision (`N` in `0..=6`).
pub(crate) fn segments(template: &str) -> Segments<'_> {
    Segments { template, pos: 0 }
}

impl<'a> Segments<'a> {
    /// Parses the placeholder whose `{` is at `self.pos`.
    fn placeholder(&mut self) -> Result<Segment<'a>, SignalTextError> {
        let at = self.pos;
        let body = at + 1;
        let tail = &self.template[body..];
        match tail.find(['{', '}']) {
            Some(i) if tail.as_bytes()[i] == b'}' => {
                self.pos = body + i + 1;
                parse_placeholder(&tail[..i], at).map(Segment::Placeholder)
            }
            Some(i) => {
                self.pos = body + i;
                Err(SignalTextError::UnclosedBrace { at })
            }
            None => {
                self.pos = self.template.len();
                Err(SignalTextError::UnclosedBrace { at })
            }
        }
    }
}

impl<'a> Iterator for Segments<'a> {
    type Item = Result<Segment<'a>, SignalTextError>;

    fn next(&mut self) -> Option<Self::Item> {
        let at = self.pos;
        let rest = &self.template[at..];
        let &first = rest.as_bytes().first()?;
        let doubled = rest.as_bytes().get(1) == Some(&first);
        Some(match first {
            b'{' | b'}' if doubled => {
                self.pos += 2;
                Ok(Segment::Literal(&rest[..1]))
            }
            b'{' => self.placeholder(),
            b'}' => {
                self.pos += 1;
                Err(SignalTextError::StrayCloseBrace { at })
            }
            _ => {
                let run = rest.find(['{', '}']).unwrap_or(rest.len());
                self.pos += run;
                Ok(Segment::Literal(&rest[..run]))
            }
        })
    }
}

/// Parses the text between a placeholder's braces. `at` is the offset of its `{`.
fn parse_placeholder(inner: &str, at: usize) -> Result<Placeholder<'_>, SignalTextError> {
    let (source, rest) = match inner.strip_prefix("self.") {
        Some(rest) => (PlaceholderSource::Entity, rest),
        None => (PlaceholderSource::World, inner),
    };
    // Only a suffix after the last colon that starts with `.` is a spec; any other colon is
    // part of the key (`group_count:enemies`).
    let (key, precision) = match rest.rfind(':') {
        Some(i) if rest[i + 1..].starts_with('.') => match &rest.as_bytes()[i + 2..] {
            &[digit @ b'0'..=b'6'] => (&rest[..i], Some(digit - b'0')),
            _ => return Err(SignalTextError::BadPrecision { at }),
        },
        _ => (rest, None),
    };
    if key.is_empty() {
        return Err(SignalTextError::EmptyKey { at });
    }
    Ok(Placeholder {
        source,
        key,
        precision,
    })
}

/// Checks that `template` is well formed: balanced braces and valid placeholders.
///
/// Returns the first problem found. Keys are not looked up, so a placeholder naming a signal
/// that does not exist is valid.
pub fn validate_signal_text(template: &str) -> Result<(), SignalTextError> {
    segments(template).try_for_each(|segment| segment.map(drop))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Result<Segment<'_>, SignalTextError> {
        Ok(Segment::Literal(s))
    }

    fn ph(
        source: PlaceholderSource,
        key: &str,
        precision: Option<u8>,
    ) -> Result<Segment<'_>, SignalTextError> {
        Ok(Segment::Placeholder(Placeholder {
            source,
            key,
            precision,
        }))
    }

    fn parse(template: &str) -> Vec<Result<Segment<'_>, SignalTextError>> {
        segments(template).collect()
    }

    fn first_error(template: &str) -> SignalTextError {
        segments(template)
            .find_map(Result::err)
            .expect("template should be malformed")
    }

    #[test]
    fn literal_only_template_is_one_literal() {
        assert_eq!(parse("Hello there"), vec![lit("Hello there")]);
        assert_eq!(parse(""), Vec::<Result<Segment, SignalTextError>>::new());
        assert_eq!(validate_signal_text("Hello there"), Ok(()));
    }

    #[test]
    fn doubled_braces_are_escapes() {
        let text: String = parse("a{{b}}c")
            .into_iter()
            .map(|s| match s.unwrap() {
                Segment::Literal(l) => l,
                Segment::Placeholder(_) => panic!("escape parsed as a placeholder"),
            })
            .collect();
        assert_eq!(text, "a{b}c");
        assert_eq!(validate_signal_text("{{}}"), Ok(()));
    }

    #[test]
    fn world_placeholder() {
        assert_eq!(
            parse("I have {apples} apples"),
            vec![
                lit("I have "),
                ph(PlaceholderSource::World, "apples", None),
                lit(" apples"),
            ]
        );
    }

    #[test]
    fn self_placeholder_reads_the_entity() {
        assert_eq!(
            parse("{self.apples}"),
            vec![ph(PlaceholderSource::Entity, "apples", None)]
        );
    }

    #[test]
    fn precision_spec_is_parsed() {
        assert_eq!(
            parse("{k:.2}"),
            vec![ph(PlaceholderSource::World, "k", Some(2))]
        );
        assert_eq!(
            parse("{self.k:.0}"),
            vec![ph(PlaceholderSource::Entity, "k", Some(0))]
        );
        assert_eq!(
            parse("{k:.6}"),
            vec![ph(PlaceholderSource::World, "k", Some(6))]
        );
    }

    #[test]
    fn a_colon_without_dot_belongs_to_the_key() {
        assert_eq!(
            parse("{group_count:enemies}"),
            vec![ph(PlaceholderSource::World, "group_count:enemies", None)]
        );
    }

    #[test]
    fn only_the_last_colon_starts_a_spec() {
        assert_eq!(
            parse("{a:b:.1}"),
            vec![ph(PlaceholderSource::World, "a:b", Some(1))]
        );
    }

    #[test]
    fn unclosed_brace_reports_the_open_brace_offset() {
        assert_eq!(
            first_error("ab {name"),
            SignalTextError::UnclosedBrace { at: 3 }
        );
        assert_eq!(
            first_error("{a{b}"),
            SignalTextError::UnclosedBrace { at: 0 }
        );
        assert_eq!(
            validate_signal_text("x{"),
            Err(SignalTextError::UnclosedBrace { at: 1 })
        );
    }

    #[test]
    fn stray_close_brace_reports_its_offset() {
        assert_eq!(
            first_error("ab}"),
            SignalTextError::StrayCloseBrace { at: 2 }
        );
        assert_eq!(
            first_error("{{}"),
            SignalTextError::StrayCloseBrace { at: 2 }
        );
    }

    #[test]
    fn empty_key_reports_the_open_brace_offset() {
        assert_eq!(first_error("ab{}"), SignalTextError::EmptyKey { at: 2 });
        assert_eq!(first_error("{self.}"), SignalTextError::EmptyKey { at: 0 });
        assert_eq!(first_error("x {:.2}"), SignalTextError::EmptyKey { at: 2 });
    }

    #[test]
    fn bad_precision_reports_the_open_brace_offset() {
        for bad in ["{k:.7}", "{k:.12}", "{k:.x}", "{k:.}", "{k:.-1}"] {
            assert_eq!(
                first_error(bad),
                SignalTextError::BadPrecision { at: 0 },
                "{bad}"
            );
        }
        assert_eq!(
            first_error("ab{k:.7}"),
            SignalTextError::BadPrecision { at: 2 }
        );
    }

    #[test]
    fn parsing_resumes_after_a_malformed_part() {
        let parsed = parse("{}{ok}");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], Err(SignalTextError::EmptyKey { at: 0 }));
        assert_eq!(parsed[1], ph(PlaceholderSource::World, "ok", None));
    }

    #[test]
    fn error_messages_name_the_offset() {
        let msg = SignalTextError::BadPrecision { at: 4 }.to_string();
        assert!(msg.contains('4'), "{msg}");
    }
}
