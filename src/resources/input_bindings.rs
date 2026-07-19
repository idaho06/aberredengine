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

use bevy_ecs::prelude::*;
use raylib::ffi::GamepadAxis;
use raylib::ffi::GamepadButton;
use raylib::ffi::KeyboardKey;
use raylib::ffi::MouseButton;

use crate::events::input::InputAction;
use crate::protocol::raw_input::MAX_GAMEPADS;

/// Which side of zero a [`GamepadAxis`] must cross to register as "down" for
/// [`InputBinding::GamepadAxis`]. The crossing threshold itself is a single
/// engine-wide constant (`crate::systems::input::GAMEPAD_AXIS_THRESHOLD`),
/// not a per-binding field -- keeping this variant free of `f32` fields lets
/// `InputBinding` keep deriving `Eq, Hash` unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AxisDirection {
    Positive,
    Negative,
}

/// A single hardware input source that can be bound to a logical action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputBinding {
    /// A physical keyboard key.
    Keyboard(KeyboardKey),
    /// A mouse button (left, right, middle, etc.).
    MouseButton(MouseButton),
    /// A gamepad button, on pad `pad` (`0..MAX_GAMEPADS`).
    GamepadButton { pad: u8, button: GamepadButton },
    /// A gamepad axis crossing past a fixed threshold in `direction`, on pad
    /// `pad`. Lets an analog stick drive a digital action (e.g. the left
    /// stick driving `MainDirection*`) with no changes to `InputState`'s
    /// shape.
    GamepadAxis {
        pad: u8,
        axis: GamepadAxis,
        direction: AxisDirection,
    },
}

/// Runtime-configurable map from logical [`InputAction`]s to hardware bindings.
///
/// Stored as an ECS [`Resource`].  The input polling system reads this each
/// frame instead of the now-removed `key_binding` field on `BoolState`.
///
/// Supports multiple bindings per action so that, for example, both W and
/// the Up arrow key can map to the same movement action.
///
/// A fixed-size array indexed by [`InputAction::index`], not a `HashMap` --
/// bindings are read up to `InputAction::COUNT` times per backlogged input
/// sample, every sim tick (default 240Hz via `[simulation] hz`), so hashing
/// a key on every lookup was steady avoidable CPU against a map that only
/// ever changes on an explicit Lua/Rust rebind.
#[derive(Resource, Debug, Clone)]
pub struct InputBindings {
    pub map: [Vec<InputBinding>; InputAction::COUNT],
    dirty: bool,
}

impl InputBindings {
    /// Replace all current bindings for `action` with a single new `binding`.
    ///
    /// This is the typical "rebind" path: the user picks one new key and the
    /// old binding is discarded.
    pub fn rebind(&mut self, action: InputAction, binding: InputBinding) {
        self.map[action.index()] = vec![binding];
        self.dirty = true;
    }

    /// Append `binding` to the list of bindings for `action` without removing
    /// existing ones (multi-bind / combo support).
    pub fn add_binding(&mut self, action: InputAction, binding: InputBinding) {
        self.map[action.index()].push(binding);
        self.dirty = true;
    }

    /// Returns whether bindings changed since the last cache refresh and clears the flag.
    pub fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    /// Return all bindings registered for `action`, or an empty slice if none.
    pub fn get_bindings(&self, action: InputAction) -> &[InputBinding] {
        self.map[action.index()].as_slice()
    }

    /// Return the first binding for `action` as a string, or `None` if unbound.
    ///
    /// Useful for displaying "current key" in a settings screen.
    pub fn first_binding_str(&self, action: InputAction) -> Option<String> {
        self.get_bindings(action)
            .first()
            .map(|b| binding_to_str(*b))
    }

    /// Iterate every action alongside its bindings. Keeps the flat-array
    /// backing storage (see the struct doc) encapsulated -- callers that
    /// need to walk every action (e.g. `update_bindings_cache`, which builds
    /// the Lua-facing snapshot) get an iterator instead of reaching into
    /// `map` and `InputAction::ALL` themselves.
    pub fn iter(&self) -> impl Iterator<Item = (InputAction, &[InputBinding])> {
        InputAction::ALL
            .into_iter()
            .map(|action| (action, self.get_bindings(action)))
    }
}

impl Default for InputBindings {
    /// The engine's default key assignments for every input action.
    fn default() -> Self {
        let k = |key: KeyboardKey| InputBinding::Keyboard(key);
        let m = |btn: MouseButton| InputBinding::MouseButton(btn);
        let mut map: [Vec<InputBinding>; InputAction::COUNT] = std::array::from_fn(|_| Vec::new());

        map[InputAction::MainDirectionUp.index()] = vec![k(KeyboardKey::KEY_W)];
        map[InputAction::MainDirectionDown.index()] = vec![k(KeyboardKey::KEY_S)];
        map[InputAction::MainDirectionLeft.index()] = vec![k(KeyboardKey::KEY_A)];
        map[InputAction::MainDirectionRight.index()] = vec![k(KeyboardKey::KEY_D)];
        map[InputAction::SecondaryDirectionUp.index()] = vec![k(KeyboardKey::KEY_UP)];
        map[InputAction::SecondaryDirectionDown.index()] = vec![k(KeyboardKey::KEY_DOWN)];
        map[InputAction::SecondaryDirectionLeft.index()] = vec![k(KeyboardKey::KEY_LEFT)];
        map[InputAction::SecondaryDirectionRight.index()] = vec![k(KeyboardKey::KEY_RIGHT)];
        map[InputAction::Back.index()] = vec![k(KeyboardKey::KEY_ESCAPE)];
        map[InputAction::Action1.index()] =
            vec![k(KeyboardKey::KEY_SPACE), m(MouseButton::MOUSE_BUTTON_LEFT)];
        map[InputAction::Action2.index()] = vec![
            k(KeyboardKey::KEY_ENTER),
            m(MouseButton::MOUSE_BUTTON_RIGHT),
        ];
        map[InputAction::Action3.index()] = vec![m(MouseButton::MOUSE_BUTTON_MIDDLE)];
        map[InputAction::Special.index()] = vec![k(KeyboardKey::KEY_F12)];
        map[InputAction::ToggleDebug.index()] = vec![k(KeyboardKey::KEY_F11)];
        map[InputAction::ToggleFullscreen.index()] = vec![k(KeyboardKey::KEY_F10)];

        let mut bindings = Self { map, dirty: true };
        bindings.add_pad0_defaults();
        bindings
    }
}

impl InputBindings {
    /// Additively bind pad 0's d-pad + left stick to BOTH the
    /// `MainDirection*` and `SecondaryDirection*` actions, face buttons to
    /// `Action1/2/3`, and start to `Back` -- keyboard/mouse defaults are
    /// untouched, this only appends. Called once from `Default::default`.
    ///
    /// Bound to both direction tiers (not just `MainDirection*`) because,
    /// unlike keyboard (which has two independent devices -- WASD and
    /// arrows -- feeding Main/Secondary separately), a gamepad has exactly
    /// one directional input. Rust systems that read a specific tier
    /// directly rather than the OR'd `digital.up/down/left/right` combined
    /// value -- e.g. `menu_controller_observer`
    /// (`src/systems/menu.rs`), which only listens for
    /// `InputAction::SecondaryDirectionUp/Down` -- would otherwise never see
    /// gamepad d-pad/stick input at all.
    fn add_pad0_defaults(&mut self) {
        let btn = |b: GamepadButton| InputBinding::GamepadButton { pad: 0, button: b };
        let axis = |a: GamepadAxis, d: AxisDirection| InputBinding::GamepadAxis {
            pad: 0,
            axis: a,
            direction: d,
        };

        let direction_defaults = [
            (
                GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP,
                GamepadAxis::GAMEPAD_AXIS_LEFT_Y,
                AxisDirection::Negative,
                InputAction::MainDirectionUp,
                InputAction::SecondaryDirectionUp,
            ),
            (
                GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN,
                GamepadAxis::GAMEPAD_AXIS_LEFT_Y,
                AxisDirection::Positive,
                InputAction::MainDirectionDown,
                InputAction::SecondaryDirectionDown,
            ),
            (
                GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT,
                GamepadAxis::GAMEPAD_AXIS_LEFT_X,
                AxisDirection::Negative,
                InputAction::MainDirectionLeft,
                InputAction::SecondaryDirectionLeft,
            ),
            (
                GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT,
                GamepadAxis::GAMEPAD_AXIS_LEFT_X,
                AxisDirection::Positive,
                InputAction::MainDirectionRight,
                InputAction::SecondaryDirectionRight,
            ),
        ];
        for (button, gamepad_axis, direction, main, secondary) in direction_defaults {
            self.add_binding(main, btn(button));
            self.add_binding(main, axis(gamepad_axis, direction));
            self.add_binding(secondary, btn(button));
            self.add_binding(secondary, axis(gamepad_axis, direction));
        }

        let defaults = [
            (
                InputAction::Action1,
                btn(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN),
            ),
            (
                InputAction::Action2,
                btn(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
            ),
            (
                InputAction::Action3,
                btn(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT),
            ),
            (
                InputAction::Back,
                btn(GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT),
            ),
        ];
        for (action, binding) in defaults {
            self.add_binding(action, binding);
        }
    }
}

/// Look up `name` in a canonical `(name, value)` table -- shared scan logic
/// behind [`key_from_str`], [`gamepad_button_from_str`], and
/// [`gamepad_axis_from_str`].
fn table_lookup<T: Copy>(table: &[(&'static str, T)], name: &str) -> Option<T> {
    table.iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
}

/// Reverse-look-up `value` in a canonical `(name, value)` table, or
/// `"unknown"` if absent -- shared scan logic behind [`key_to_str`],
/// [`gamepad_button_to_str`], and [`gamepad_axis_to_str`].
fn table_reverse<T: Copy + PartialEq>(table: &[(&'static str, T)], value: T) -> &'static str {
    table
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(s, _)| *s)
        .unwrap_or("unknown")
}

// ---------------------------------------------------------------------------
// Key ↔ string conversion helpers
// ---------------------------------------------------------------------------

/// Canonical (non-alias) name ↔ key table, the single source of truth for
/// both [`key_from_str`] and [`key_to_str`]. Alias names (`"return"`,
/// `"esc"`, `"shift"`, `"ctrl"`) are handled separately since they have no
/// corresponding canonical-name entry to round-trip to.
const KEY_NAME_TABLE: &[(&str, KeyboardKey)] = &[
    // Letters
    ("a", KeyboardKey::KEY_A),
    ("b", KeyboardKey::KEY_B),
    ("c", KeyboardKey::KEY_C),
    ("d", KeyboardKey::KEY_D),
    ("e", KeyboardKey::KEY_E),
    ("f", KeyboardKey::KEY_F),
    ("g", KeyboardKey::KEY_G),
    ("h", KeyboardKey::KEY_H),
    ("i", KeyboardKey::KEY_I),
    ("j", KeyboardKey::KEY_J),
    ("k", KeyboardKey::KEY_K),
    ("l", KeyboardKey::KEY_L),
    ("m", KeyboardKey::KEY_M),
    ("n", KeyboardKey::KEY_N),
    ("o", KeyboardKey::KEY_O),
    ("p", KeyboardKey::KEY_P),
    ("q", KeyboardKey::KEY_Q),
    ("r", KeyboardKey::KEY_R),
    ("s", KeyboardKey::KEY_S),
    ("t", KeyboardKey::KEY_T),
    ("u", KeyboardKey::KEY_U),
    ("v", KeyboardKey::KEY_V),
    ("w", KeyboardKey::KEY_W),
    ("x", KeyboardKey::KEY_X),
    ("y", KeyboardKey::KEY_Y),
    ("z", KeyboardKey::KEY_Z),
    // Digits
    ("0", KeyboardKey::KEY_ZERO),
    ("1", KeyboardKey::KEY_ONE),
    ("2", KeyboardKey::KEY_TWO),
    ("3", KeyboardKey::KEY_THREE),
    ("4", KeyboardKey::KEY_FOUR),
    ("5", KeyboardKey::KEY_FIVE),
    ("6", KeyboardKey::KEY_SIX),
    ("7", KeyboardKey::KEY_SEVEN),
    ("8", KeyboardKey::KEY_EIGHT),
    ("9", KeyboardKey::KEY_NINE),
    // Special
    ("space", KeyboardKey::KEY_SPACE),
    ("enter", KeyboardKey::KEY_ENTER),
    ("escape", KeyboardKey::KEY_ESCAPE),
    ("backspace", KeyboardKey::KEY_BACKSPACE),
    ("tab", KeyboardKey::KEY_TAB),
    // Arrows
    ("up", KeyboardKey::KEY_UP),
    ("down", KeyboardKey::KEY_DOWN),
    ("left", KeyboardKey::KEY_LEFT),
    ("right", KeyboardKey::KEY_RIGHT),
    // Modifiers
    ("lshift", KeyboardKey::KEY_LEFT_SHIFT),
    ("rshift", KeyboardKey::KEY_RIGHT_SHIFT),
    ("lctrl", KeyboardKey::KEY_LEFT_CONTROL),
    ("rctrl", KeyboardKey::KEY_RIGHT_CONTROL),
    ("lalt", KeyboardKey::KEY_LEFT_ALT),
    ("ralt", KeyboardKey::KEY_RIGHT_ALT),
    // Function keys
    ("f1", KeyboardKey::KEY_F1),
    ("f2", KeyboardKey::KEY_F2),
    ("f3", KeyboardKey::KEY_F3),
    ("f4", KeyboardKey::KEY_F4),
    ("f5", KeyboardKey::KEY_F5),
    ("f6", KeyboardKey::KEY_F6),
    ("f7", KeyboardKey::KEY_F7),
    ("f8", KeyboardKey::KEY_F8),
    ("f9", KeyboardKey::KEY_F9),
    ("f10", KeyboardKey::KEY_F10),
    ("f11", KeyboardKey::KEY_F11),
    ("f12", KeyboardKey::KEY_F12),
];

/// Parse a human-readable key name into a [`KeyboardKey`].
///
/// Returns `None` for unknown names. Names are lowercase, e.g. `"w"`, `"space"`,
/// `"f11"`. Common aliases (`"return"` → `KEY_ENTER`, `"esc"` → `KEY_ESCAPE`) are
/// accepted.
pub fn key_from_str(s: &str) -> Option<KeyboardKey> {
    table_lookup(KEY_NAME_TABLE, s).or(match s {
        "return" => Some(KeyboardKey::KEY_ENTER),
        "esc" => Some(KeyboardKey::KEY_ESCAPE),
        "shift" => Some(KeyboardKey::KEY_LEFT_SHIFT),
        "ctrl" => Some(KeyboardKey::KEY_LEFT_CONTROL),
        _ => None,
    })
}

/// Serialize a [`KeyboardKey`] to a canonical lowercase string.
///
/// Returns `"unknown"` for keys not covered by the mapping.
pub fn key_to_str(k: KeyboardKey) -> &'static str {
    table_reverse(KEY_NAME_TABLE, k)
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

// ---------------------------------------------------------------------------
// Gamepad ↔ string conversion helpers
// ---------------------------------------------------------------------------

/// Canonical gamepad button name ↔ `GamepadButton` table. Named
/// *positionally* (`face_up`/`face_down`/...) rather than by Xbox-style
/// `a`/`b`/`x`/`y` labels, since those imply a specific controller layout
/// that could mislead e.g. PlayStation-pad users and is awkward to change
/// later without breaking saved rebind strings.
const GAMEPAD_BUTTON_NAME_TABLE: &[(&str, GamepadButton)] = &[
    ("dpad_up", GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP),
    ("dpad_right", GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT),
    ("dpad_down", GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN),
    ("dpad_left", GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT),
    ("face_up", GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP),
    ("face_right", GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT),
    ("face_down", GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN),
    ("face_left", GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT),
    ("lb", GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1),
    ("lt", GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2),
    ("rb", GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1),
    ("rt", GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2),
    ("select", GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT),
    ("guide", GamepadButton::GAMEPAD_BUTTON_MIDDLE),
    ("start", GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT),
    ("left_thumb", GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB),
    ("right_thumb", GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB),
];

/// Canonical gamepad axis name ↔ `GamepadAxis` table (used with a trailing
/// `+`/`-` direction suffix, e.g. `"axis_lx+"`).
const GAMEPAD_AXIS_NAME_TABLE: &[(&str, GamepadAxis)] = &[
    ("axis_lx", GamepadAxis::GAMEPAD_AXIS_LEFT_X),
    ("axis_ly", GamepadAxis::GAMEPAD_AXIS_LEFT_Y),
    ("axis_rx", GamepadAxis::GAMEPAD_AXIS_RIGHT_X),
    ("axis_ry", GamepadAxis::GAMEPAD_AXIS_RIGHT_Y),
    ("axis_lt", GamepadAxis::GAMEPAD_AXIS_LEFT_TRIGGER),
    ("axis_rt", GamepadAxis::GAMEPAD_AXIS_RIGHT_TRIGGER),
];

fn gamepad_button_from_str(s: &str) -> Option<GamepadButton> {
    table_lookup(GAMEPAD_BUTTON_NAME_TABLE, s)
}

fn gamepad_button_to_str(b: GamepadButton) -> &'static str {
    table_reverse(GAMEPAD_BUTTON_NAME_TABLE, b)
}

fn gamepad_axis_from_str(s: &str) -> Option<GamepadAxis> {
    table_lookup(GAMEPAD_AXIS_NAME_TABLE, s)
}

fn gamepad_axis_to_str(a: GamepadAxis) -> &'static str {
    table_reverse(GAMEPAD_AXIS_NAME_TABLE, a)
}

/// Parse a `"padN:..."` gamepad binding string (e.g. `"pad0:face_down"`,
/// `"pad0:dpad_up"`, `"pad0:axis_lx+"`) into an [`InputBinding`]. `N` must be
/// a valid pad index (`< MAX_GAMEPADS`). Returns `None` on any parse failure.
fn gamepad_binding_from_str(s: &str) -> Option<InputBinding> {
    let rest = s.strip_prefix("pad")?;
    let (pad_str, name) = rest.split_once(':')?;
    let pad: u8 = pad_str.parse().ok()?;
    if pad as usize >= MAX_GAMEPADS {
        return None;
    }

    if let Some((axis_name, dir_ch)) = name
        .strip_suffix('+')
        .map(|n| (n, '+'))
        .or_else(|| name.strip_suffix('-').map(|n| (n, '-')))
    {
        let axis = gamepad_axis_from_str(axis_name)?;
        let direction = if dir_ch == '+' {
            AxisDirection::Positive
        } else {
            AxisDirection::Negative
        };
        return Some(InputBinding::GamepadAxis {
            pad,
            axis,
            direction,
        });
    }

    gamepad_button_from_str(name).map(|button| InputBinding::GamepadButton { pad, button })
}

fn gamepad_binding_to_str(pad: u8, rest: &str) -> String {
    format!("pad{pad}:{rest}")
}

/// Parse any binding string into an [`InputBinding`].
///
/// Tries mouse button names first (`"mouse_left"`, etc.), then keyboard key
/// names, then the `"padN:..."` gamepad grammar. Returns `None` for unknown
/// strings.
pub fn binding_from_str(s: &str) -> Option<InputBinding> {
    if let Some(m) = mouse_button_from_str(s) {
        return Some(InputBinding::MouseButton(m));
    }
    if let Some(k) = key_from_str(s) {
        return Some(InputBinding::Keyboard(k));
    }
    gamepad_binding_from_str(s)
}

/// Serialize an [`InputBinding`] to a canonical string. Returns an owned
/// `String` rather than `&'static str` because gamepad bindings are
/// parametric on `pad` (`"pad0:..."`, `"pad1:..."`) and can't be
/// pre-enumerated as `'static` literals.
pub fn binding_to_str(b: InputBinding) -> String {
    match b {
        InputBinding::Keyboard(k) => key_to_str(k).to_string(),
        InputBinding::MouseButton(m) => mouse_button_to_str(m).to_string(),
        InputBinding::GamepadButton { pad, button } => {
            gamepad_binding_to_str(pad, gamepad_button_to_str(button))
        }
        InputBinding::GamepadAxis {
            pad,
            axis,
            direction,
        } => {
            let sign = match direction {
                AxisDirection::Positive => "+",
                AxisDirection::Negative => "-",
            };
            gamepad_binding_to_str(pad, &format!("{}{sign}", gamepad_axis_to_str(axis)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_action_index_is_a_bijection_onto_0_count() {
        // Every ALL entry must map to a distinct index < COUNT -- catches
        // copy-paste errors in InputAction::index's manual match (e.g. two
        // variants accidentally sharing a slot, silently corrupting both
        // actions' bindings).
        let mut seen = [false; InputAction::COUNT];
        for action in InputAction::ALL {
            let i = action.index();
            assert!(i < InputAction::COUNT, "{action:?} index {i} out of range");
            assert!(
                !seen[i],
                "{action:?} index {i} collides with another variant"
            );
            seen[i] = true;
        }
        assert!(
            seen.iter().all(|&s| s),
            "every slot 0..COUNT must be reachable"
        );
    }

    #[test]
    fn test_default_bindings_are_correct() {
        // Keyboard/mouse defaults are always the FIRST bindings for each
        // action -- pad-0 gamepad defaults (see test_pad0_defaults_are_appended
        // below) are additive, appended after `Default::default`'s
        // keyboard/mouse `map.insert` calls.
        let b = InputBindings::default();
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionUp)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_W)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionDown)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_S)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionLeft)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_A)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionRight)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_D)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionUp)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_UP)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionDown)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_DOWN)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionLeft)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_LEFT)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionRight)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_RIGHT)
        );
        assert_eq!(
            b.get_bindings(InputAction::Back)[0],
            InputBinding::Keyboard(KeyboardKey::KEY_ESCAPE)
        );
        assert_eq!(
            &b.get_bindings(InputAction::Action1)[..2],
            &[
                InputBinding::Keyboard(KeyboardKey::KEY_SPACE),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT),
            ]
        );
        assert_eq!(
            &b.get_bindings(InputAction::Action2)[..2],
            &[
                InputBinding::Keyboard(KeyboardKey::KEY_ENTER),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_RIGHT),
            ]
        );
        assert_eq!(
            b.get_bindings(InputAction::Action3)[0],
            InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE)
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
    fn test_pad0_defaults_are_appended() {
        let b = InputBindings::default();
        assert!(b.get_bindings(InputAction::MainDirectionUp).contains(
            &InputBinding::GamepadButton {
                pad: 0,
                button: GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP,
            }
        ));
        assert!(b.get_bindings(InputAction::MainDirectionUp).contains(
            &InputBinding::GamepadAxis {
                pad: 0,
                axis: GamepadAxis::GAMEPAD_AXIS_LEFT_Y,
                direction: AxisDirection::Negative,
            }
        ));
        assert!(
            b.get_bindings(InputAction::Action1)
                .contains(&InputBinding::GamepadButton {
                    pad: 0,
                    button: GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
                })
        );
        assert!(
            b.get_bindings(InputAction::Back)
                .contains(&InputBinding::GamepadButton {
                    pad: 0,
                    button: GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT,
                })
        );
        // SecondaryDirectionUp gets the SAME d-pad/stick additions as
        // MainDirectionUp -- a gamepad has one directional input, unlike
        // keyboard's two independent devices (WASD vs arrows), and Rust
        // systems like menu_controller_observer read SecondaryDirection*
        // directly rather than the OR'd combined value.
        assert!(b.get_bindings(InputAction::SecondaryDirectionUp).contains(
            &InputBinding::GamepadButton {
                pad: 0,
                button: GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP,
            }
        ));
        assert!(b.get_bindings(InputAction::SecondaryDirectionUp).contains(
            &InputBinding::GamepadAxis {
                pad: 0,
                axis: GamepadAxis::GAMEPAD_AXIS_LEFT_Y,
                direction: AxisDirection::Negative,
            }
        ));
        // Untouched actions get no pad-0 additions.
        assert_eq!(b.get_bindings(InputAction::Special).len(), 1);
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
        // default: Space + MouseLeft + pad0 face_down, plus new Z appended last
        assert_eq!(bl[0], InputBinding::Keyboard(KeyboardKey::KEY_SPACE));
        assert_eq!(
            bl[1],
            InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)
        );
        assert_eq!(
            *bl.last().unwrap(),
            InputBinding::Keyboard(KeyboardKey::KEY_Z)
        );
    }

    #[test]
    fn test_get_bindings_unregistered_returns_empty() {
        let b = InputBindings {
            map: std::array::from_fn(|_| Vec::new()),
            dirty: false,
        };
        assert!(b.get_bindings(InputAction::Action1).is_empty());
    }

    #[test]
    fn test_first_binding_str_returns_canonical_name() {
        let b = InputBindings::default();
        assert_eq!(
            b.first_binding_str(InputAction::Action1).as_deref(),
            Some("space")
        );
        assert_eq!(
            b.first_binding_str(InputAction::MainDirectionUp).as_deref(),
            Some("w")
        );
        assert_eq!(
            b.first_binding_str(InputAction::ToggleDebug).as_deref(),
            Some("f11")
        );
    }

    #[test]
    fn test_first_binding_str_unbound_returns_none() {
        let b = InputBindings {
            map: std::array::from_fn(|_| Vec::new()),
            dirty: false,
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
        // Action3 default is mouse_middle first (pad0 face_left appended after)
        assert_eq!(
            b.first_binding_str(InputAction::Action3).as_deref(),
            Some("mouse_middle")
        );
    }

    #[test]
    fn test_gamepad_binding_from_str_button_and_axis_roundtrip() {
        assert_eq!(
            binding_from_str("pad0:face_down"),
            Some(InputBinding::GamepadButton {
                pad: 0,
                button: GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
            })
        );
        assert_eq!(
            binding_to_str(InputBinding::GamepadButton {
                pad: 0,
                button: GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN,
            }),
            "pad0:face_down"
        );
        assert_eq!(
            binding_from_str("pad0:axis_lx+"),
            Some(InputBinding::GamepadAxis {
                pad: 0,
                axis: GamepadAxis::GAMEPAD_AXIS_LEFT_X,
                direction: AxisDirection::Positive,
            })
        );
        assert_eq!(
            binding_to_str(InputBinding::GamepadAxis {
                pad: 0,
                axis: GamepadAxis::GAMEPAD_AXIS_LEFT_X,
                direction: AxisDirection::Positive,
            }),
            "pad0:axis_lx+"
        );
        assert_eq!(
            binding_from_str("pad1:axis_ry-"),
            Some(InputBinding::GamepadAxis {
                pad: 1,
                axis: GamepadAxis::GAMEPAD_AXIS_RIGHT_Y,
                direction: AxisDirection::Negative,
            })
        );
    }

    #[test]
    fn test_gamepad_binding_from_str_out_of_range_pad_returns_none() {
        assert_eq!(binding_from_str("pad9:face_down"), None);
    }

    #[test]
    fn test_gamepad_binding_from_str_unknown_name_returns_none() {
        assert_eq!(binding_from_str("pad0:not_a_button"), None);
        assert_eq!(binding_from_str("pad0:axis_zz+"), None);
        assert_eq!(binding_from_str("padx:face_down"), None);
    }
}
