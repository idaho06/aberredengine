//! Signal value to text lookup shared by signal bindings and dialogue placeholders.
//!
//! `signal_as_str` reads one signal key from any signal source (`WorldSignals`,
//! `SignalSnapshot`, or an entity's `Signals` component) and renders the value as
//! text: integers and scalars through `Display`, strings borrowed as-is, and set
//! flags as `"true"`.
//!
//! The module also parses placeholder templates (`{key}`, `{self.key}`, `{key:.2}`):
//! `format_signal_text` fills one in, `validate_signal_text` checks one,
//! `for_each_missing_signal_key` lists the keys that have no value, and
//! `SignalTextError` says what is wrong with a malformed template.

use arrayvec::ArrayString;
use std::borrow::Cow;
use std::fmt::Write as _;

use crate::components::signals::Signals;
use crate::resources::world_signals::SignalsRead;

/// Capacity for a formatted number. `f32`'s `Display` never uses exponent notation, so its
/// widest output is `-f32::MAX` at 40 chars (`i32::MIN` is 11). A buffer that is too small
/// makes `write!` fail part-way and the text shows a truncated number.
const NUM_BUF_LEN: usize = 48;

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

/// Renders `args` into a stack buffer. The widest number, `-f32::MAX` with the largest
/// precision of 6, still fits in [`NUM_BUF_LEN`].
fn write_stack<'a>(args: std::fmt::Arguments<'_>) -> SignalStr<'a> {
    let mut buf = ArrayString::<NUM_BUF_LEN>::new();
    let _ = buf.write_fmt(args);
    SignalStr::Stack(buf)
}

/// Converts a signal value to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag. `precision` is the number
/// of digits after the point for numbers (an integer prints as a float then); strings and
/// flags ignore it. Returns `None` if the signal key is not found.
pub(crate) fn signal_as_str<'a, S: SignalLookup + ?Sized>(
    src: &'a S,
    key: &str,
    precision: Option<u8>,
) -> Option<SignalStr<'a>> {
    if let Some(v) = src.get_integer(key) {
        return Some(match precision {
            Some(n) => write_stack(format_args!("{:.*}", usize::from(n), f64::from(v))),
            None => write_stack(format_args!("{v}")),
        });
    }
    if let Some(v) = src.get_scalar(key) {
        return Some(match precision {
            Some(n) => write_stack(format_args!("{:.*}", usize::from(n), v)),
            None => write_stack(format_args!("{v}")),
        });
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

/// Marks a placeholder as reading the entity `Signals`: `{self.key}`.
const SELF_PREFIX: &str = "self.";

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
    /// The key as authored: with its `self.` prefix, without the spec.
    pub(crate) written: &'a str,
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
    let (source, rest) = match inner.strip_prefix(SELF_PREFIX) {
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
    let written = &inner[..inner.len() - rest.len() + key.len()];
    Ok(Placeholder {
        source,
        written,
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

/// Looks up the value a placeholder names in its own source.
fn resolve<'a>(
    placeholder: &Placeholder<'_>,
    world: &'a dyn SignalsRead,
    entity: Option<&'a Signals>,
) -> Option<SignalStr<'a>> {
    match placeholder.source {
        PlaceholderSource::World => signal_as_str(world, placeholder.key, placeholder.precision),
        PlaceholderSource::Entity => signal_as_str(entity?, placeholder.key, placeholder.precision),
    }
}

/// Fills the placeholders of `template` from `world` and returns the text.
///
/// `{key}` reads `world`; `{key:.N}` prints a number with `N` digits after the point. A key
/// that is not found keeps its literal token (`{key:.2}` included), and a malformed part is
/// copied through as it was written, so this never fails and never panics. A template with no
/// brace at all is returned borrowed. Call [`validate_signal_text`] to reject a malformed
/// template up front.
///
/// `entity` is the signals that `{self.key}` reads; without one, every `{self.key}` is
/// missing. There is no fallback between the two sources.
pub fn format_signal_text<'a>(
    template: &'a str,
    world: &dyn SignalsRead,
    entity: Option<&Signals>,
) -> Cow<'a, str> {
    if !template.contains(['{', '}']) {
        return Cow::Borrowed(template);
    }
    let mut out = String::with_capacity(template.len());
    let mut parts = segments(template);
    loop {
        let start = parts.pos;
        let Some(part) = parts.next() else { break };
        let value = match part {
            Ok(Segment::Literal(text)) => {
                out.push_str(text);
                continue;
            }
            Ok(Segment::Placeholder(p)) => resolve(&p, world, entity),
            Err(_) => None,
        };
        match value {
            Some(v) => out.push_str(v.as_str()),
            // Missing key or malformed part: keep the source text.
            None => out.push_str(&template[start..parts.pos]),
        }
    }
    Cow::Owned(out)
}

/// Calls `f` with the key of every placeholder in `template` that has no value, in order and
/// as written (`self.key` keeps its prefix). A key used twice is reported twice.
///
/// A malformed template reports nothing: [`validate_signal_text`] is the check for those.
pub fn for_each_missing_signal_key(
    template: &str,
    world: &dyn SignalsRead,
    entity: Option<&Signals>,
    mut f: impl FnMut(&str),
) {
    if validate_signal_text(template).is_err() {
        return;
    }
    for part in segments(template) {
        let Ok(Segment::Placeholder(p)) = part else {
            continue;
        };
        if resolve(&p, world, entity).is_none() {
            f(p.written);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Result<Segment<'_>, SignalTextError> {
        Ok(Segment::Literal(s))
    }

    fn ph<'a>(
        source: PlaceholderSource,
        written: &'a str,
        key: &'a str,
        precision: Option<u8>,
    ) -> Result<Segment<'a>, SignalTextError> {
        Ok(Segment::Placeholder(Placeholder {
            source,
            written,
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
                ph(PlaceholderSource::World, "apples", "apples", None),
                lit(" apples"),
            ]
        );
    }

    #[test]
    fn self_placeholder_reads_the_entity() {
        assert_eq!(
            parse("{self.apples}"),
            vec![ph(PlaceholderSource::Entity, "self.apples", "apples", None)]
        );
    }

    #[test]
    fn precision_spec_is_parsed() {
        assert_eq!(
            parse("{k:.2}"),
            vec![ph(PlaceholderSource::World, "k", "k", Some(2))]
        );
        assert_eq!(
            parse("{self.k:.0}"),
            vec![ph(PlaceholderSource::Entity, "self.k", "k", Some(0))]
        );
        assert_eq!(
            parse("{k:.6}"),
            vec![ph(PlaceholderSource::World, "k", "k", Some(6))]
        );
    }

    #[test]
    fn a_colon_without_dot_belongs_to_the_key() {
        assert_eq!(
            parse("{group_count:enemies}"),
            vec![ph(
                PlaceholderSource::World,
                "group_count:enemies",
                "group_count:enemies",
                None
            )]
        );
    }

    #[test]
    fn only_the_last_colon_starts_a_spec() {
        assert_eq!(
            parse("{a:b:.1}"),
            vec![ph(PlaceholderSource::World, "a:b", "a:b", Some(1))]
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
        assert_eq!(parsed[1], ph(PlaceholderSource::World, "ok", "ok", None));
    }

    #[test]
    fn error_messages_name_the_offset() {
        let msg = SignalTextError::BadPrecision { at: 4 }.to_string();
        assert!(msg.contains('4'), "{msg}");
    }

    mod format {
        use super::super::*;
        use crate::resources::world_signals::WorldSignals;
        use std::borrow::Cow;

        fn world() -> WorldSignals {
            let mut w = WorldSignals::default();
            w.set_integer("apples", 6);
            w.set_scalar("grain", 5.0);
            w.set_scalar("price", 2.5);
            w.set_string("player_name", "Idaho06");
            w.set_flag("open");
            w
        }

        fn fmt(template: &str) -> String {
            format_signal_text(template, &world(), None).into_owned()
        }

        #[test]
        fn fills_each_signal_type() {
            assert_eq!(
                fmt("{apples} apples, {grain} kg, {price} gold, {player_name}, open={open}"),
                "6 apples, 5 kg, 2.5 gold, Idaho06, open=true"
            );
        }

        #[test]
        fn lookup_order_is_integer_scalar_string_flag() {
            let mut w = WorldSignals::default();
            w.set_flag("k");
            assert_eq!(format_signal_text("{k}", &w, None), "true");
            w.set_string("k", "text");
            assert_eq!(format_signal_text("{k}", &w, None), "text");
            w.set_scalar("k", 1.5);
            assert_eq!(format_signal_text("{k}", &w, None), "1.5");
            w.set_integer("k", 7);
            assert_eq!(format_signal_text("{k}", &w, None), "7");
        }

        #[test]
        fn precision_applies_to_integers_and_scalars() {
            assert_eq!(fmt("{apples:.2}"), "6.00");
            assert_eq!(fmt("{apples:.0}"), "6");
            assert_eq!(fmt("{price:.0}"), "2");
            assert_eq!(fmt("{price:.3}"), "2.500");
            assert_eq!(fmt("{grain:.1}"), "5.0");
        }

        #[test]
        fn precision_on_a_string_or_flag_is_ignored() {
            assert_eq!(fmt("{player_name:.2} {open:.3}"), "Idaho06 true");
        }

        #[test]
        fn a_missing_key_keeps_its_literal_token() {
            assert_eq!(fmt("Hi {nobody}!"), "Hi {nobody}!");
            assert_eq!(fmt("{nobody:.2}"), "{nobody:.2}");
            assert_eq!(fmt("{self.apples}"), "{self.apples}");
        }

        #[test]
        fn escapes_become_single_braces() {
            assert_eq!(fmt("{{{apples}}}"), "{6}");
        }

        #[test]
        fn malformed_parts_are_copied_through_literally() {
            assert_eq!(fmt("a } b {apples}"), "a } b 6");
            assert_eq!(fmt("{} {apples}"), "{} 6");
            assert_eq!(fmt("{apples:.9} {apples}"), "{apples:.9} 6");
            assert_eq!(fmt("{apples"), "{apples");
            assert_eq!(fmt("{a{apples}"), "{a6");
        }

        #[test]
        fn a_template_without_braces_is_borrowed() {
            let out = format_signal_text("no placeholders here", &world(), None);
            assert!(matches!(out, Cow::Borrowed("no placeholders here")));
        }

        #[test]
        fn world_signals_and_a_snapshot_format_identically() {
            let mut w = world();
            let snapshot = w.snapshot();
            let template = "{apples} {grain:.2} {player_name} {open} {nobody}";
            assert_eq!(
                format_signal_text(template, &w, None),
                format_signal_text(template, &*snapshot, None)
            );
        }

        #[test]
        fn the_widest_number_prints_in_full() {
            let mut w = WorldSignals::default();
            w.set_scalar("big", -f32::MAX);
            let expected = format!("{:.6}", -f32::MAX);
            assert_eq!(expected.len(), 47);
            assert_eq!(format_signal_text("{big:.6}", &w, None), expected);
        }
    }

    mod entity {
        use super::super::*;
        use crate::resources::world_signals::WorldSignals;

        fn world() -> WorldSignals {
            let mut w = WorldSignals::default();
            w.set_integer("shared", 1);
            w.set_string("player_name", "Idaho06");
            w
        }

        fn entity() -> Signals {
            let mut e = Signals::default();
            e.set_integer("shared", 2);
            e.set_integer("apples", 6);
            e.set_scalar("grain", 5.0);
            e
        }

        fn missing(template: &str, entity: Option<&Signals>) -> Vec<String> {
            let mut keys = Vec::new();
            for_each_missing_signal_key(template, &world(), entity, |k| keys.push(k.to_string()));
            keys
        }

        #[test]
        fn self_reads_only_the_entity_and_plain_reads_only_the_world() {
            let (w, e) = (world(), entity());
            let out = format_signal_text("{shared} {self.shared}", &w, Some(&e));
            assert_eq!(out, "1 2");
        }

        #[test]
        fn self_applies_precision() {
            let (w, e) = (world(), entity());
            let out = format_signal_text("{self.apples:.1} {self.grain:.2}", &w, Some(&e));
            assert_eq!(out, "6.0 5.00");
        }

        #[test]
        fn self_without_an_entity_is_missing_and_keeps_its_token() {
            let w = world();
            assert_eq!(
                format_signal_text("{self.apples}", &w, None),
                "{self.apples}"
            );
        }

        #[test]
        fn self_does_not_fall_back_to_the_world() {
            let (w, e) = (world(), entity());
            let out = format_signal_text("{self.player_name} {apples}", &w, Some(&e));
            assert_eq!(out, "{self.player_name} {apples}");
        }

        #[test]
        fn the_scan_yields_each_missing_key_in_order() {
            let e = entity();
            let keys = missing(
                "{nobody} {player_name} {self.apples} {self.ghost:.2} {other}",
                Some(&e),
            );
            assert_eq!(keys, ["nobody", "self.ghost", "other"]);
        }

        #[test]
        fn the_scan_counts_self_keys_as_missing_without_an_entity() {
            assert_eq!(missing("{self.apples}", None), ["self.apples"]);
        }

        #[test]
        fn the_scan_yields_nothing_for_a_malformed_template() {
            assert!(missing("{nobody} {", None).is_empty());
            assert!(missing("{} {nobody}", None).is_empty());
        }

        #[test]
        fn the_scan_yields_nothing_when_everything_resolves() {
            let e = entity();
            assert!(missing("{player_name} {self.apples} {{ok}}", Some(&e)).is_empty());
        }
    }
}
