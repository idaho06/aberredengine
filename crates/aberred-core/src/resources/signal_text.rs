//! Signal value to text lookup shared by signal bindings and dialogue placeholders.
//!
//! `signal_as_str` reads one signal key from any signal source (`WorldSignals`,
//! `SignalSnapshot`, or an entity's `Signals` component) and renders the value as
//! text: integers and scalars through `Display`, strings borrowed as-is, and set
//! flags as `"true"`.

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
