//! Runtime-configurable input binding resource.
//!
//! [`InputBindings`] decouples the *hardware key* triggering an action from the
//! *current frame state* of that action (which remains in [`InputState`]).
//! Each logical [`InputAction`] maps to one or more [`InputBinding`] values,
//! allowing runtime rebinding from Rust or Lua.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Rebind Action1 from Space to Z
//! bindings.rebind(InputAction::Action1, InputBinding::Keyboard(KeyboardKey::KEY_Z));
//!
//! // Add a second binding (multi-bind: both Z and X trigger Action1)
//! bindings.add_binding(InputAction::Action1, InputBinding::Keyboard(KeyboardKey::KEY_X));
//!
//! // Read bindings in the input polling system
//! let keys = bindings.get_bindings(InputAction::Action1);
//! ```

use std::collections::HashMap;

use bevy_ecs::prelude::*;
use raylib::ffi::KeyboardKey;
use raylib::ffi::MouseButton;

use crate::events::input::InputAction;

/// A single hardware input source that can be bound to a logical action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputBinding {
    /// A physical keyboard key.
    Keyboard(KeyboardKey),
    /// A mouse button (left, right, middle, etc.).
    MouseButton(MouseButton),
}

/// Runtime-configurable map from logical [`InputAction`]s to hardware bindings.
///
/// Stored as an ECS [`Resource`].  The input polling system reads this each
/// frame instead of the now-removed `key_binding` field on `BoolState`.
///
/// Supports multiple bindings per action so that, for example, both W and
/// the Up arrow key can map to the same movement action.
#[derive(Resource, Debug, Clone)]
pub struct InputBindings {
    pub map: HashMap<InputAction, Vec<InputBinding>>,
}

impl InputBindings {
    /// Replace all current bindings for `action` with a single new `binding`.
    ///
    /// This is the typical "rebind" path: the user picks one new key and the
    /// old binding is discarded.
    pub fn rebind(&mut self, action: InputAction, binding: InputBinding) {
        self.map.insert(action, vec![binding]);
    }

    /// Append `binding` to the list of bindings for `action` without removing
    /// existing ones (multi-bind / combo support).
    pub fn add_binding(&mut self, action: InputAction, binding: InputBinding) {
        self.map.entry(action).or_default().push(binding);
    }

    /// Return all bindings registered for `action`, or an empty slice if none.
    pub fn get_bindings(&self, action: InputAction) -> &[InputBinding] {
        self.map.get(&action).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Return the first binding for `action` as a string, or `None` if unbound.
    ///
    /// Useful for displaying "current key" in a settings screen.
    pub fn first_binding_str(&self, action: InputAction) -> Option<&'static str> {
        self.get_bindings(action).first().map(|b| match b {
            InputBinding::Keyboard(k) => key_to_str(*k),
            InputBinding::MouseButton(m) => mouse_button_to_str(*m),
        })
    }
}

impl Default for InputBindings {
    /// Mirrors the default key assignments that were previously hardcoded into
    /// `BoolState::key_binding` fields on `InputState`.
    fn default() -> Self {
        let k = |key: KeyboardKey| InputBinding::Keyboard(key);
        let m = |btn: MouseButton| InputBinding::MouseButton(btn);
        let mut map = HashMap::new();

        map.insert(InputAction::MainDirectionUp, vec![k(KeyboardKey::KEY_W)]);
        map.insert(InputAction::MainDirectionDown, vec![k(KeyboardKey::KEY_S)]);
        map.insert(InputAction::MainDirectionLeft, vec![k(KeyboardKey::KEY_A)]);
        map.insert(InputAction::MainDirectionRight, vec![k(KeyboardKey::KEY_D)]);
        map.insert(
            InputAction::SecondaryDirectionUp,
            vec![k(KeyboardKey::KEY_UP)],
        );
        map.insert(
            InputAction::SecondaryDirectionDown,
            vec![k(KeyboardKey::KEY_DOWN)],
        );
        map.insert(
            InputAction::SecondaryDirectionLeft,
            vec![k(KeyboardKey::KEY_LEFT)],
        );
        map.insert(
            InputAction::SecondaryDirectionRight,
            vec![k(KeyboardKey::KEY_RIGHT)],
        );
        map.insert(InputAction::Back, vec![k(KeyboardKey::KEY_ESCAPE)]);
        map.insert(
            InputAction::Action1,
            vec![k(KeyboardKey::KEY_SPACE), m(MouseButton::MOUSE_BUTTON_LEFT)],
        );
        map.insert(
            InputAction::Action2,
            vec![
                k(KeyboardKey::KEY_ENTER),
                m(MouseButton::MOUSE_BUTTON_RIGHT),
            ],
        );
        map.insert(
            InputAction::Action3,
            vec![m(MouseButton::MOUSE_BUTTON_MIDDLE)],
        );
        map.insert(InputAction::Special, vec![k(KeyboardKey::KEY_F12)]);
        map.insert(InputAction::ToggleDebug, vec![k(KeyboardKey::KEY_F11)]);
        map.insert(InputAction::ToggleFullscreen, vec![k(KeyboardKey::KEY_F10)]);

        Self { map }
    }
}

// ---------------------------------------------------------------------------
// Key ↔ string conversion helpers
// ---------------------------------------------------------------------------

/// Parse a human-readable key name into a [`KeyboardKey`].
///
/// Returns `None` for unknown names. Names are lowercase, e.g. `"w"`, `"space"`,
/// `"f11"`. Common aliases (`"return"` → `KEY_ENTER`, `"esc"` → `KEY_ESCAPE`) are
/// accepted.
pub fn key_from_str(s: &str) -> Option<KeyboardKey> {
    match s {
        // Letters
        "a" => Some(KeyboardKey::KEY_A),
        "b" => Some(KeyboardKey::KEY_B),
        "c" => Some(KeyboardKey::KEY_C),
        "d" => Some(KeyboardKey::KEY_D),
        "e" => Some(KeyboardKey::KEY_E),
        "f" => Some(KeyboardKey::KEY_F),
        "g" => Some(KeyboardKey::KEY_G),
        "h" => Some(KeyboardKey::KEY_H),
        "i" => Some(KeyboardKey::KEY_I),
        "j" => Some(KeyboardKey::KEY_J),
        "k" => Some(KeyboardKey::KEY_K),
        "l" => Some(KeyboardKey::KEY_L),
        "m" => Some(KeyboardKey::KEY_M),
        "n" => Some(KeyboardKey::KEY_N),
        "o" => Some(KeyboardKey::KEY_O),
        "p" => Some(KeyboardKey::KEY_P),
        "q" => Some(KeyboardKey::KEY_Q),
        "r" => Some(KeyboardKey::KEY_R),
        "s" => Some(KeyboardKey::KEY_S),
        "t" => Some(KeyboardKey::KEY_T),
        "u" => Some(KeyboardKey::KEY_U),
        "v" => Some(KeyboardKey::KEY_V),
        "w" => Some(KeyboardKey::KEY_W),
        "x" => Some(KeyboardKey::KEY_X),
        "y" => Some(KeyboardKey::KEY_Y),
        "z" => Some(KeyboardKey::KEY_Z),
        // Digits
        "0" => Some(KeyboardKey::KEY_ZERO),
        "1" => Some(KeyboardKey::KEY_ONE),
        "2" => Some(KeyboardKey::KEY_TWO),
        "3" => Some(KeyboardKey::KEY_THREE),
        "4" => Some(KeyboardKey::KEY_FOUR),
        "5" => Some(KeyboardKey::KEY_FIVE),
        "6" => Some(KeyboardKey::KEY_SIX),
        "7" => Some(KeyboardKey::KEY_SEVEN),
        "8" => Some(KeyboardKey::KEY_EIGHT),
        "9" => Some(KeyboardKey::KEY_NINE),
        // Special
        "space" => Some(KeyboardKey::KEY_SPACE),
        "enter" | "return" => Some(KeyboardKey::KEY_ENTER),
        "escape" | "esc" => Some(KeyboardKey::KEY_ESCAPE),
        "backspace" => Some(KeyboardKey::KEY_BACKSPACE),
        "tab" => Some(KeyboardKey::KEY_TAB),
        // Arrows
        "up" => Some(KeyboardKey::KEY_UP),
        "down" => Some(KeyboardKey::KEY_DOWN),
        "left" => Some(KeyboardKey::KEY_LEFT),
        "right" => Some(KeyboardKey::KEY_RIGHT),
        // Modifiers
        "lshift" | "shift" => Some(KeyboardKey::KEY_LEFT_SHIFT),
        "rshift" => Some(KeyboardKey::KEY_RIGHT_SHIFT),
        "lctrl" | "ctrl" => Some(KeyboardKey::KEY_LEFT_CONTROL),
        "rctrl" => Some(KeyboardKey::KEY_RIGHT_CONTROL),
        "lalt" | "alt" => Some(KeyboardKey::KEY_LEFT_ALT),
        "ralt" => Some(KeyboardKey::KEY_RIGHT_ALT),
        // Function keys
        "f1" => Some(KeyboardKey::KEY_F1),
        "f2" => Some(KeyboardKey::KEY_F2),
        "f3" => Some(KeyboardKey::KEY_F3),
        "f4" => Some(KeyboardKey::KEY_F4),
        "f5" => Some(KeyboardKey::KEY_F5),
        "f6" => Some(KeyboardKey::KEY_F6),
        "f7" => Some(KeyboardKey::KEY_F7),
        "f8" => Some(KeyboardKey::KEY_F8),
        "f9" => Some(KeyboardKey::KEY_F9),
        "f10" => Some(KeyboardKey::KEY_F10),
        "f11" => Some(KeyboardKey::KEY_F11),
        "f12" => Some(KeyboardKey::KEY_F12),
        _ => None,
    }
}

/// Serialize a [`KeyboardKey`] to a canonical lowercase string.
///
/// Returns `"unknown"` for keys not covered by the mapping.
pub fn key_to_str(k: KeyboardKey) -> &'static str {
    match k {
        // Letters
        KeyboardKey::KEY_A => "a",
        KeyboardKey::KEY_B => "b",
        KeyboardKey::KEY_C => "c",
        KeyboardKey::KEY_D => "d",
        KeyboardKey::KEY_E => "e",
        KeyboardKey::KEY_F => "f",
        KeyboardKey::KEY_G => "g",
        KeyboardKey::KEY_H => "h",
        KeyboardKey::KEY_I => "i",
        KeyboardKey::KEY_J => "j",
        KeyboardKey::KEY_K => "k",
        KeyboardKey::KEY_L => "l",
        KeyboardKey::KEY_M => "m",
        KeyboardKey::KEY_N => "n",
        KeyboardKey::KEY_O => "o",
        KeyboardKey::KEY_P => "p",
        KeyboardKey::KEY_Q => "q",
        KeyboardKey::KEY_R => "r",
        KeyboardKey::KEY_S => "s",
        KeyboardKey::KEY_T => "t",
        KeyboardKey::KEY_U => "u",
        KeyboardKey::KEY_V => "v",
        KeyboardKey::KEY_W => "w",
        KeyboardKey::KEY_X => "x",
        KeyboardKey::KEY_Y => "y",
        KeyboardKey::KEY_Z => "z",
        // Digits
        KeyboardKey::KEY_ZERO => "0",
        KeyboardKey::KEY_ONE => "1",
        KeyboardKey::KEY_TWO => "2",
        KeyboardKey::KEY_THREE => "3",
        KeyboardKey::KEY_FOUR => "4",
        KeyboardKey::KEY_FIVE => "5",
        KeyboardKey::KEY_SIX => "6",
        KeyboardKey::KEY_SEVEN => "7",
        KeyboardKey::KEY_EIGHT => "8",
        KeyboardKey::KEY_NINE => "9",
        // Special
        KeyboardKey::KEY_SPACE => "space",
        KeyboardKey::KEY_ENTER => "enter",
        KeyboardKey::KEY_ESCAPE => "escape",
        KeyboardKey::KEY_BACKSPACE => "backspace",
        KeyboardKey::KEY_TAB => "tab",
        // Arrows
        KeyboardKey::KEY_UP => "up",
        KeyboardKey::KEY_DOWN => "down",
        KeyboardKey::KEY_LEFT => "left",
        KeyboardKey::KEY_RIGHT => "right",
        // Modifiers
        KeyboardKey::KEY_LEFT_SHIFT => "lshift",
        KeyboardKey::KEY_RIGHT_SHIFT => "rshift",
        KeyboardKey::KEY_LEFT_CONTROL => "lctrl",
        KeyboardKey::KEY_RIGHT_CONTROL => "rctrl",
        KeyboardKey::KEY_LEFT_ALT => "lalt",
        KeyboardKey::KEY_RIGHT_ALT => "ralt",
        // Function keys
        KeyboardKey::KEY_F1 => "f1",
        KeyboardKey::KEY_F2 => "f2",
        KeyboardKey::KEY_F3 => "f3",
        KeyboardKey::KEY_F4 => "f4",
        KeyboardKey::KEY_F5 => "f5",
        KeyboardKey::KEY_F6 => "f6",
        KeyboardKey::KEY_F7 => "f7",
        KeyboardKey::KEY_F8 => "f8",
        KeyboardKey::KEY_F9 => "f9",
        KeyboardKey::KEY_F10 => "f10",
        KeyboardKey::KEY_F11 => "f11",
        KeyboardKey::KEY_F12 => "f12",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Mouse button ↔ string conversion helpers
// ---------------------------------------------------------------------------

/// Parse a mouse button name into a [`MouseButton`].
///
/// Accepted names: `"mouse_left"`, `"mouse_right"`, `"mouse_middle"`.
pub fn mouse_button_from_str(s: &str) -> Option<MouseButton> {
    match s {
        "mouse_left" => Some(MouseButton::MOUSE_BUTTON_LEFT),
        "mouse_right" => Some(MouseButton::MOUSE_BUTTON_RIGHT),
        "mouse_middle" => Some(MouseButton::MOUSE_BUTTON_MIDDLE),
        _ => None,
    }
}

/// Serialize a [`MouseButton`] to a canonical lowercase string.
pub fn mouse_button_to_str(m: MouseButton) -> &'static str {
    match m {
        MouseButton::MOUSE_BUTTON_LEFT => "mouse_left",
        MouseButton::MOUSE_BUTTON_RIGHT => "mouse_right",
        MouseButton::MOUSE_BUTTON_MIDDLE => "mouse_middle",
        _ => "mouse_unknown",
    }
}

/// Parse any binding string into an [`InputBinding`].
///
/// Tries mouse button names first (`"mouse_left"`, etc.), then keyboard key names.
/// Returns `None` for unknown strings.
pub fn binding_from_str(s: &str) -> Option<InputBinding> {
    if let Some(m) = mouse_button_from_str(s) {
        return Some(InputBinding::MouseButton(m));
    }
    key_from_str(s).map(InputBinding::Keyboard)
}

/// Serialize an [`InputBinding`] to a canonical string.
pub fn binding_to_str(b: InputBinding) -> &'static str {
    match b {
        InputBinding::Keyboard(k) => key_to_str(k),
        InputBinding::MouseButton(m) => mouse_button_to_str(m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings_are_correct() {
        let b = InputBindings::default();
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionUp),
            &[InputBinding::Keyboard(KeyboardKey::KEY_W)]
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionDown),
            &[InputBinding::Keyboard(KeyboardKey::KEY_S)]
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionLeft),
            &[InputBinding::Keyboard(KeyboardKey::KEY_A)]
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionRight),
            &[InputBinding::Keyboard(KeyboardKey::KEY_D)]
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionUp),
            &[InputBinding::Keyboard(KeyboardKey::KEY_UP)]
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionDown),
            &[InputBinding::Keyboard(KeyboardKey::KEY_DOWN)]
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionLeft),
            &[InputBinding::Keyboard(KeyboardKey::KEY_LEFT)]
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionRight),
            &[InputBinding::Keyboard(KeyboardKey::KEY_RIGHT)]
        );
        assert_eq!(
            b.get_bindings(InputAction::Back),
            &[InputBinding::Keyboard(KeyboardKey::KEY_ESCAPE)]
        );
        assert_eq!(
            b.get_bindings(InputAction::Action1),
            &[
                InputBinding::Keyboard(KeyboardKey::KEY_SPACE),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT),
            ]
        );
        assert_eq!(
            b.get_bindings(InputAction::Action2),
            &[
                InputBinding::Keyboard(KeyboardKey::KEY_ENTER),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_RIGHT),
            ]
        );
        assert_eq!(
            b.get_bindings(InputAction::Action3),
            &[InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE)]
        );
        assert_eq!(
            b.get_bindings(InputAction::Special),
            &[InputBinding::Keyboard(KeyboardKey::KEY_F12)]
        );
        assert_eq!(
            b.get_bindings(InputAction::ToggleDebug),
            &[InputBinding::Keyboard(KeyboardKey::KEY_F11)]
        );
        assert_eq!(
            b.get_bindings(InputAction::ToggleFullscreen),
            &[InputBinding::Keyboard(KeyboardKey::KEY_F10)]
        );
    }

    #[test]
    fn test_rebind_replaces_binding() {
        let mut b = InputBindings::default();
        b.rebind(
            InputAction::Action1,
            InputBinding::Keyboard(KeyboardKey::KEY_Z),
        );
        let bl = b.get_bindings(InputAction::Action1);
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0], InputBinding::Keyboard(KeyboardKey::KEY_Z));
    }

    #[test]
    fn test_add_binding_appends() {
        let mut b = InputBindings::default();
        b.add_binding(
            InputAction::Action1,
            InputBinding::Keyboard(KeyboardKey::KEY_Z),
        );
        let bl = b.get_bindings(InputAction::Action1);
        // default: Space + MouseLeft, plus new Z
        assert_eq!(bl.len(), 3);
        assert_eq!(bl[0], InputBinding::Keyboard(KeyboardKey::KEY_SPACE));
        assert_eq!(
            bl[1],
            InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)
        );
        assert_eq!(bl[2], InputBinding::Keyboard(KeyboardKey::KEY_Z));
    }

    #[test]
    fn test_get_bindings_unregistered_returns_empty() {
        let b = InputBindings {
            map: HashMap::new(),
        };
        assert!(b.get_bindings(InputAction::Action1).is_empty());
    }

    #[test]
    fn test_first_binding_str_returns_canonical_name() {
        let b = InputBindings::default();
        assert_eq!(b.first_binding_str(InputAction::Action1), Some("space"));
        assert_eq!(b.first_binding_str(InputAction::MainDirectionUp), Some("w"));
        assert_eq!(b.first_binding_str(InputAction::ToggleDebug), Some("f11"));
    }

    #[test]
    fn test_first_binding_str_unbound_returns_none() {
        let b = InputBindings {
            map: HashMap::new(),
        };
        assert_eq!(b.first_binding_str(InputAction::Action1), None);
    }

    #[test]
    fn test_key_from_str_roundtrip() {
        let pairs: &[(&str, KeyboardKey)] = &[
            ("w", KeyboardKey::KEY_W),
            ("a", KeyboardKey::KEY_A),
            ("s", KeyboardKey::KEY_S),
            ("d", KeyboardKey::KEY_D),
            ("space", KeyboardKey::KEY_SPACE),
            ("enter", KeyboardKey::KEY_ENTER),
            ("escape", KeyboardKey::KEY_ESCAPE),
            ("up", KeyboardKey::KEY_UP),
            ("down", KeyboardKey::KEY_DOWN),
            ("left", KeyboardKey::KEY_LEFT),
            ("right", KeyboardKey::KEY_RIGHT),
            ("f10", KeyboardKey::KEY_F10),
            ("f11", KeyboardKey::KEY_F11),
            ("f12", KeyboardKey::KEY_F12),
            ("z", KeyboardKey::KEY_Z),
            ("backspace", KeyboardKey::KEY_BACKSPACE),
            ("tab", KeyboardKey::KEY_TAB),
            ("lshift", KeyboardKey::KEY_LEFT_SHIFT),
            ("lctrl", KeyboardKey::KEY_LEFT_CONTROL),
        ];
        for (name, key) in pairs {
            assert_eq!(
                key_from_str(name),
                Some(*key),
                "key_from_str(\"{}\") did not return {:?}",
                name,
                key
            );
            assert_eq!(
                key_to_str(*key),
                *name,
                "key_to_str({:?}) did not return \"{}\"",
                key,
                name
            );
        }
    }

    #[test]
    fn test_key_from_str_aliases() {
        // "return" is an alias for "enter"
        assert_eq!(key_from_str("return"), Some(KeyboardKey::KEY_ENTER));
        // "esc" is an alias for "escape"
        assert_eq!(key_from_str("esc"), Some(KeyboardKey::KEY_ESCAPE));
        // "shift" is an alias for "lshift"
        assert_eq!(key_from_str("shift"), Some(KeyboardKey::KEY_LEFT_SHIFT));
        // "ctrl" is an alias for "lctrl"
        assert_eq!(key_from_str("ctrl"), Some(KeyboardKey::KEY_LEFT_CONTROL));
    }

    #[test]
    fn test_key_from_str_unknown_returns_none() {
        assert_eq!(key_from_str(""), None);
        assert_eq!(key_from_str("numpad0"), None);
        assert_eq!(key_from_str("SPACE"), None); // case-sensitive
    }

    #[test]
    fn test_mouse_button_from_str_roundtrip() {
        let pairs = [
            ("mouse_left", MouseButton::MOUSE_BUTTON_LEFT),
            ("mouse_right", MouseButton::MOUSE_BUTTON_RIGHT),
            ("mouse_middle", MouseButton::MOUSE_BUTTON_MIDDLE),
        ];
        for (name, btn) in pairs {
            assert_eq!(mouse_button_from_str(name), Some(btn));
            assert_eq!(mouse_button_to_str(btn), name);
        }
    }

    #[test]
    fn test_mouse_button_from_str_unknown_returns_none() {
        assert_eq!(mouse_button_from_str(""), None);
        assert_eq!(mouse_button_from_str("left"), None);
        assert_eq!(mouse_button_from_str("mouse_4"), None);
    }

    #[test]
    fn test_binding_from_str_keyboard() {
        assert_eq!(
            binding_from_str("space"),
            Some(InputBinding::Keyboard(KeyboardKey::KEY_SPACE))
        );
        assert_eq!(
            binding_from_str("w"),
            Some(InputBinding::Keyboard(KeyboardKey::KEY_W))
        );
    }

    #[test]
    fn test_binding_from_str_mouse() {
        assert_eq!(
            binding_from_str("mouse_left"),
            Some(InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT))
        );
        assert_eq!(
            binding_from_str("mouse_right"),
            Some(InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_RIGHT))
        );
        assert_eq!(
            binding_from_str("mouse_middle"),
            Some(InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE))
        );
    }

    #[test]
    fn test_binding_from_str_unknown_returns_none() {
        assert_eq!(binding_from_str(""), None);
        assert_eq!(binding_from_str("not_a_binding"), None);
    }

    #[test]
    fn test_binding_to_str_keyboard() {
        assert_eq!(
            binding_to_str(InputBinding::Keyboard(KeyboardKey::KEY_SPACE)),
            "space"
        );
    }

    #[test]
    fn test_binding_to_str_mouse() {
        assert_eq!(
            binding_to_str(InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)),
            "mouse_left"
        );
    }

    #[test]
    fn test_first_binding_str_mouse_button() {
        let b = InputBindings::default();
        // Action3 default is mouse_middle only
        assert_eq!(
            b.first_binding_str(InputAction::Action3),
            Some("mouse_middle")
        );
    }
}
