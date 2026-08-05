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
//! bindings.rebind(InputAction::Action1, InputBinding::Keyboard(Key::KEY_Z));
//!
//! // Add a second binding (multi-bind: both Z and X trigger Action1)
//! bindings.add_binding(InputAction::Action1, InputBinding::Keyboard(Key::KEY_X));
//!
//! // Read bindings in the input polling system
//! let keys = bindings.get_bindings(InputAction::Action1);
//! ```

use bevy_ecs::prelude::*;

use crate::events::input::InputAction;
use crate::protocol::raw_input::MAX_GAMEPADS;

/// Engine-owned hardware keyboard key code. Numeric values match raylib's
/// `KeyboardKey` codes (GLFW-derived) 1:1 -- verified against this project's
/// vendored `sola-raylib-sys` FFI bindings, not transcribed from memory. A
/// raylib version bump that renumbers keys would be caught by the exhaustive
/// parity check in `tests/input_types_raylib_parity.rs`. No exhaustive match is
/// expected: an unmapped `u16` is representable but simply never equals any
/// `KEY_*` const, mirroring raylib's own permissive int-based key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key(pub u16);

impl Key {
    pub const KEY_NULL: Key = Key(0);
    pub const KEY_APOSTROPHE: Key = Key(39);
    pub const KEY_COMMA: Key = Key(44);
    pub const KEY_MINUS: Key = Key(45);
    pub const KEY_PERIOD: Key = Key(46);
    pub const KEY_SLASH: Key = Key(47);
    pub const KEY_ZERO: Key = Key(48);
    pub const KEY_ONE: Key = Key(49);
    pub const KEY_TWO: Key = Key(50);
    pub const KEY_THREE: Key = Key(51);
    pub const KEY_FOUR: Key = Key(52);
    pub const KEY_FIVE: Key = Key(53);
    pub const KEY_SIX: Key = Key(54);
    pub const KEY_SEVEN: Key = Key(55);
    pub const KEY_EIGHT: Key = Key(56);
    pub const KEY_NINE: Key = Key(57);
    pub const KEY_SEMICOLON: Key = Key(59);
    pub const KEY_EQUAL: Key = Key(61);
    pub const KEY_A: Key = Key(65);
    pub const KEY_B: Key = Key(66);
    pub const KEY_C: Key = Key(67);
    pub const KEY_D: Key = Key(68);
    pub const KEY_E: Key = Key(69);
    pub const KEY_F: Key = Key(70);
    pub const KEY_G: Key = Key(71);
    pub const KEY_H: Key = Key(72);
    pub const KEY_I: Key = Key(73);
    pub const KEY_J: Key = Key(74);
    pub const KEY_K: Key = Key(75);
    pub const KEY_L: Key = Key(76);
    pub const KEY_M: Key = Key(77);
    pub const KEY_N: Key = Key(78);
    pub const KEY_O: Key = Key(79);
    pub const KEY_P: Key = Key(80);
    pub const KEY_Q: Key = Key(81);
    pub const KEY_R: Key = Key(82);
    pub const KEY_S: Key = Key(83);
    pub const KEY_T: Key = Key(84);
    pub const KEY_U: Key = Key(85);
    pub const KEY_V: Key = Key(86);
    pub const KEY_W: Key = Key(87);
    pub const KEY_X: Key = Key(88);
    pub const KEY_Y: Key = Key(89);
    pub const KEY_Z: Key = Key(90);
    pub const KEY_LEFT_BRACKET: Key = Key(91);
    pub const KEY_BACKSLASH: Key = Key(92);
    pub const KEY_RIGHT_BRACKET: Key = Key(93);
    pub const KEY_GRAVE: Key = Key(96);
    pub const KEY_SPACE: Key = Key(32);
    pub const KEY_ESCAPE: Key = Key(256);
    pub const KEY_ENTER: Key = Key(257);
    pub const KEY_TAB: Key = Key(258);
    pub const KEY_BACKSPACE: Key = Key(259);
    pub const KEY_INSERT: Key = Key(260);
    pub const KEY_DELETE: Key = Key(261);
    pub const KEY_RIGHT: Key = Key(262);
    pub const KEY_LEFT: Key = Key(263);
    pub const KEY_DOWN: Key = Key(264);
    pub const KEY_UP: Key = Key(265);
    pub const KEY_PAGE_UP: Key = Key(266);
    pub const KEY_PAGE_DOWN: Key = Key(267);
    pub const KEY_HOME: Key = Key(268);
    pub const KEY_END: Key = Key(269);
    pub const KEY_CAPS_LOCK: Key = Key(280);
    pub const KEY_SCROLL_LOCK: Key = Key(281);
    pub const KEY_NUM_LOCK: Key = Key(282);
    pub const KEY_PRINT_SCREEN: Key = Key(283);
    pub const KEY_PAUSE: Key = Key(284);
    pub const KEY_F1: Key = Key(290);
    pub const KEY_F2: Key = Key(291);
    pub const KEY_F3: Key = Key(292);
    pub const KEY_F4: Key = Key(293);
    pub const KEY_F5: Key = Key(294);
    pub const KEY_F6: Key = Key(295);
    pub const KEY_F7: Key = Key(296);
    pub const KEY_F8: Key = Key(297);
    pub const KEY_F9: Key = Key(298);
    pub const KEY_F10: Key = Key(299);
    pub const KEY_F11: Key = Key(300);
    pub const KEY_F12: Key = Key(301);
    pub const KEY_LEFT_SHIFT: Key = Key(340);
    pub const KEY_LEFT_CONTROL: Key = Key(341);
    pub const KEY_LEFT_ALT: Key = Key(342);
    pub const KEY_LEFT_SUPER: Key = Key(343);
    pub const KEY_RIGHT_SHIFT: Key = Key(344);
    pub const KEY_RIGHT_CONTROL: Key = Key(345);
    pub const KEY_RIGHT_ALT: Key = Key(346);
    pub const KEY_RIGHT_SUPER: Key = Key(347);
    pub const KEY_KB_MENU: Key = Key(348);
    pub const KEY_KP_0: Key = Key(320);
    pub const KEY_KP_1: Key = Key(321);
    pub const KEY_KP_2: Key = Key(322);
    pub const KEY_KP_3: Key = Key(323);
    pub const KEY_KP_4: Key = Key(324);
    pub const KEY_KP_5: Key = Key(325);
    pub const KEY_KP_6: Key = Key(326);
    pub const KEY_KP_7: Key = Key(327);
    pub const KEY_KP_8: Key = Key(328);
    pub const KEY_KP_9: Key = Key(329);
    pub const KEY_KP_DECIMAL: Key = Key(330);
    pub const KEY_KP_DIVIDE: Key = Key(331);
    pub const KEY_KP_MULTIPLY: Key = Key(332);
    pub const KEY_KP_SUBTRACT: Key = Key(333);
    pub const KEY_KP_ADD: Key = Key(334);
    pub const KEY_KP_ENTER: Key = Key(335);
    pub const KEY_KP_EQUAL: Key = Key(336);
    pub const KEY_BACK: Key = Key(4);
    pub const KEY_MENU: Key = Key(5);
    pub const KEY_VOLUME_UP: Key = Key(24);
    pub const KEY_VOLUME_DOWN: Key = Key(25);

    /// Widen to the `u32` code width `RawDeviceSnapshot`'s keyboard bitset
    /// indexes with.
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

/// Engine-owned mouse button code. Numeric values match raylib's
/// `MouseButton` codes 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MouseButton(pub u16);

impl MouseButton {
    pub const MOUSE_BUTTON_LEFT: MouseButton = MouseButton(0);
    pub const MOUSE_BUTTON_RIGHT: MouseButton = MouseButton(1);
    pub const MOUSE_BUTTON_MIDDLE: MouseButton = MouseButton(2);
    pub const MOUSE_BUTTON_SIDE: MouseButton = MouseButton(3);
    pub const MOUSE_BUTTON_EXTRA: MouseButton = MouseButton(4);
    pub const MOUSE_BUTTON_FORWARD: MouseButton = MouseButton(5);
    pub const MOUSE_BUTTON_BACK: MouseButton = MouseButton(6);

    /// Widen to the `u8` code width `RawDeviceSnapshot`'s mouse-button
    /// bitmask indexes with.
    pub const fn as_u8(self) -> u8 {
        self.0 as u8
    }
}

/// Engine-owned gamepad button code. Numeric values match raylib's
/// `GamepadButton` codes 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GamepadButton(pub u16);

impl GamepadButton {
    pub const GAMEPAD_BUTTON_UNKNOWN: GamepadButton = GamepadButton(0);
    pub const GAMEPAD_BUTTON_LEFT_FACE_UP: GamepadButton = GamepadButton(1);
    pub const GAMEPAD_BUTTON_LEFT_FACE_RIGHT: GamepadButton = GamepadButton(2);
    pub const GAMEPAD_BUTTON_LEFT_FACE_DOWN: GamepadButton = GamepadButton(3);
    pub const GAMEPAD_BUTTON_LEFT_FACE_LEFT: GamepadButton = GamepadButton(4);
    pub const GAMEPAD_BUTTON_RIGHT_FACE_UP: GamepadButton = GamepadButton(5);
    pub const GAMEPAD_BUTTON_RIGHT_FACE_RIGHT: GamepadButton = GamepadButton(6);
    pub const GAMEPAD_BUTTON_RIGHT_FACE_DOWN: GamepadButton = GamepadButton(7);
    pub const GAMEPAD_BUTTON_RIGHT_FACE_LEFT: GamepadButton = GamepadButton(8);
    pub const GAMEPAD_BUTTON_LEFT_TRIGGER_1: GamepadButton = GamepadButton(9);
    pub const GAMEPAD_BUTTON_LEFT_TRIGGER_2: GamepadButton = GamepadButton(10);
    pub const GAMEPAD_BUTTON_RIGHT_TRIGGER_1: GamepadButton = GamepadButton(11);
    pub const GAMEPAD_BUTTON_RIGHT_TRIGGER_2: GamepadButton = GamepadButton(12);
    pub const GAMEPAD_BUTTON_MIDDLE_LEFT: GamepadButton = GamepadButton(13);
    pub const GAMEPAD_BUTTON_MIDDLE: GamepadButton = GamepadButton(14);
    pub const GAMEPAD_BUTTON_MIDDLE_RIGHT: GamepadButton = GamepadButton(15);
    pub const GAMEPAD_BUTTON_LEFT_THUMB: GamepadButton = GamepadButton(16);
    pub const GAMEPAD_BUTTON_RIGHT_THUMB: GamepadButton = GamepadButton(17);

    /// Widen to the `u32` code width `RawGamepad`'s button bitmask indexes
    /// with.
    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

/// Engine-owned gamepad axis code. Numeric values match raylib's
/// `GamepadAxis` codes 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GamepadAxis(pub u16);

impl GamepadAxis {
    pub const GAMEPAD_AXIS_LEFT_X: GamepadAxis = GamepadAxis(0);
    pub const GAMEPAD_AXIS_LEFT_Y: GamepadAxis = GamepadAxis(1);
    pub const GAMEPAD_AXIS_RIGHT_X: GamepadAxis = GamepadAxis(2);
    pub const GAMEPAD_AXIS_RIGHT_Y: GamepadAxis = GamepadAxis(3);
    pub const GAMEPAD_AXIS_LEFT_TRIGGER: GamepadAxis = GamepadAxis(4);
    pub const GAMEPAD_AXIS_RIGHT_TRIGGER: GamepadAxis = GamepadAxis(5);

    /// Widen to the array-index width `RawGamepad::axes` is indexed with.
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

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
    Keyboard(Key),
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
        let k = |key: Key| InputBinding::Keyboard(key);
        let m = |btn: MouseButton| InputBinding::MouseButton(btn);
        let mut map: [Vec<InputBinding>; InputAction::COUNT] = std::array::from_fn(|_| Vec::new());

        map[InputAction::MainDirectionUp.index()] = vec![k(Key::KEY_W)];
        map[InputAction::MainDirectionDown.index()] = vec![k(Key::KEY_S)];
        map[InputAction::MainDirectionLeft.index()] = vec![k(Key::KEY_A)];
        map[InputAction::MainDirectionRight.index()] = vec![k(Key::KEY_D)];
        map[InputAction::SecondaryDirectionUp.index()] = vec![k(Key::KEY_UP)];
        map[InputAction::SecondaryDirectionDown.index()] = vec![k(Key::KEY_DOWN)];
        map[InputAction::SecondaryDirectionLeft.index()] = vec![k(Key::KEY_LEFT)];
        map[InputAction::SecondaryDirectionRight.index()] = vec![k(Key::KEY_RIGHT)];
        map[InputAction::Back.index()] = vec![k(Key::KEY_ESCAPE)];
        map[InputAction::Action1.index()] =
            vec![k(Key::KEY_SPACE), m(MouseButton::MOUSE_BUTTON_LEFT)];
        map[InputAction::Action2.index()] = vec![
            k(Key::KEY_ENTER),
            m(MouseButton::MOUSE_BUTTON_RIGHT),
        ];
        map[InputAction::Action3.index()] = vec![m(MouseButton::MOUSE_BUTTON_MIDDLE)];
        map[InputAction::Special.index()] = vec![k(Key::KEY_F12)];
        map[InputAction::ToggleDebug.index()] = vec![k(Key::KEY_F11)];
        map[InputAction::ToggleFullscreen.index()] = vec![k(Key::KEY_F10)];

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
const KEY_NAME_TABLE: &[(&str, Key)] = &[
    // Letters
    ("a", Key::KEY_A),
    ("b", Key::KEY_B),
    ("c", Key::KEY_C),
    ("d", Key::KEY_D),
    ("e", Key::KEY_E),
    ("f", Key::KEY_F),
    ("g", Key::KEY_G),
    ("h", Key::KEY_H),
    ("i", Key::KEY_I),
    ("j", Key::KEY_J),
    ("k", Key::KEY_K),
    ("l", Key::KEY_L),
    ("m", Key::KEY_M),
    ("n", Key::KEY_N),
    ("o", Key::KEY_O),
    ("p", Key::KEY_P),
    ("q", Key::KEY_Q),
    ("r", Key::KEY_R),
    ("s", Key::KEY_S),
    ("t", Key::KEY_T),
    ("u", Key::KEY_U),
    ("v", Key::KEY_V),
    ("w", Key::KEY_W),
    ("x", Key::KEY_X),
    ("y", Key::KEY_Y),
    ("z", Key::KEY_Z),
    // Digits
    ("0", Key::KEY_ZERO),
    ("1", Key::KEY_ONE),
    ("2", Key::KEY_TWO),
    ("3", Key::KEY_THREE),
    ("4", Key::KEY_FOUR),
    ("5", Key::KEY_FIVE),
    ("6", Key::KEY_SIX),
    ("7", Key::KEY_SEVEN),
    ("8", Key::KEY_EIGHT),
    ("9", Key::KEY_NINE),
    // Special
    ("space", Key::KEY_SPACE),
    ("enter", Key::KEY_ENTER),
    ("escape", Key::KEY_ESCAPE),
    ("backspace", Key::KEY_BACKSPACE),
    ("tab", Key::KEY_TAB),
    // Arrows
    ("up", Key::KEY_UP),
    ("down", Key::KEY_DOWN),
    ("left", Key::KEY_LEFT),
    ("right", Key::KEY_RIGHT),
    // Modifiers
    ("lshift", Key::KEY_LEFT_SHIFT),
    ("rshift", Key::KEY_RIGHT_SHIFT),
    ("lctrl", Key::KEY_LEFT_CONTROL),
    ("rctrl", Key::KEY_RIGHT_CONTROL),
    ("lalt", Key::KEY_LEFT_ALT),
    ("ralt", Key::KEY_RIGHT_ALT),
    // Function keys
    ("f1", Key::KEY_F1),
    ("f2", Key::KEY_F2),
    ("f3", Key::KEY_F3),
    ("f4", Key::KEY_F4),
    ("f5", Key::KEY_F5),
    ("f6", Key::KEY_F6),
    ("f7", Key::KEY_F7),
    ("f8", Key::KEY_F8),
    ("f9", Key::KEY_F9),
    ("f10", Key::KEY_F10),
    ("f11", Key::KEY_F11),
    ("f12", Key::KEY_F12),
];

/// Parse a human-readable key name into a [`Key`].
///
/// Returns `None` for unknown names. Names are lowercase, e.g. `"w"`, `"space"`,
/// `"f11"`. Common aliases (`"return"` → `KEY_ENTER`, `"esc"` → `KEY_ESCAPE`) are
/// accepted.
pub fn key_from_str(s: &str) -> Option<Key> {
    table_lookup(KEY_NAME_TABLE, s).or(match s {
        "return" => Some(Key::KEY_ENTER),
        "esc" => Some(Key::KEY_ESCAPE),
        "shift" => Some(Key::KEY_LEFT_SHIFT),
        "ctrl" => Some(Key::KEY_LEFT_CONTROL),
        _ => None,
    })
}

/// Serialize a [`Key`] to a canonical lowercase string.
///
/// Returns `"unknown"` for keys not covered by the mapping.
pub fn key_to_str(k: Key) -> &'static str {
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
            InputBinding::Keyboard(Key::KEY_W)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionDown)[0],
            InputBinding::Keyboard(Key::KEY_S)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionLeft)[0],
            InputBinding::Keyboard(Key::KEY_A)
        );
        assert_eq!(
            b.get_bindings(InputAction::MainDirectionRight)[0],
            InputBinding::Keyboard(Key::KEY_D)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionUp)[0],
            InputBinding::Keyboard(Key::KEY_UP)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionDown)[0],
            InputBinding::Keyboard(Key::KEY_DOWN)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionLeft)[0],
            InputBinding::Keyboard(Key::KEY_LEFT)
        );
        assert_eq!(
            b.get_bindings(InputAction::SecondaryDirectionRight)[0],
            InputBinding::Keyboard(Key::KEY_RIGHT)
        );
        assert_eq!(
            b.get_bindings(InputAction::Back)[0],
            InputBinding::Keyboard(Key::KEY_ESCAPE)
        );
        assert_eq!(
            &b.get_bindings(InputAction::Action1)[..2],
            &[
                InputBinding::Keyboard(Key::KEY_SPACE),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT),
            ]
        );
        assert_eq!(
            &b.get_bindings(InputAction::Action2)[..2],
            &[
                InputBinding::Keyboard(Key::KEY_ENTER),
                InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_RIGHT),
            ]
        );
        assert_eq!(
            b.get_bindings(InputAction::Action3)[0],
            InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE)
        );
        assert_eq!(
            b.get_bindings(InputAction::Special),
            &[InputBinding::Keyboard(Key::KEY_F12)]
        );
        assert_eq!(
            b.get_bindings(InputAction::ToggleDebug),
            &[InputBinding::Keyboard(Key::KEY_F11)]
        );
        assert_eq!(
            b.get_bindings(InputAction::ToggleFullscreen),
            &[InputBinding::Keyboard(Key::KEY_F10)]
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
            InputBinding::Keyboard(Key::KEY_Z),
        );
        let bl = b.get_bindings(InputAction::Action1);
        assert_eq!(bl.len(), 1);
        assert_eq!(bl[0], InputBinding::Keyboard(Key::KEY_Z));
    }

    #[test]
    fn test_add_binding_appends() {
        let mut b = InputBindings::default();
        b.add_binding(
            InputAction::Action1,
            InputBinding::Keyboard(Key::KEY_Z),
        );
        let bl = b.get_bindings(InputAction::Action1);
        // default: Space + MouseLeft + pad0 face_down, plus new Z appended last
        assert_eq!(bl[0], InputBinding::Keyboard(Key::KEY_SPACE));
        assert_eq!(
            bl[1],
            InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)
        );
        assert_eq!(
            *bl.last().unwrap(),
            InputBinding::Keyboard(Key::KEY_Z)
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
        let pairs: &[(&str, Key)] = &[
            ("w", Key::KEY_W),
            ("a", Key::KEY_A),
            ("s", Key::KEY_S),
            ("d", Key::KEY_D),
            ("space", Key::KEY_SPACE),
            ("enter", Key::KEY_ENTER),
            ("escape", Key::KEY_ESCAPE),
            ("up", Key::KEY_UP),
            ("down", Key::KEY_DOWN),
            ("left", Key::KEY_LEFT),
            ("right", Key::KEY_RIGHT),
            ("f10", Key::KEY_F10),
            ("f11", Key::KEY_F11),
            ("f12", Key::KEY_F12),
            ("z", Key::KEY_Z),
            ("backspace", Key::KEY_BACKSPACE),
            ("tab", Key::KEY_TAB),
            ("lshift", Key::KEY_LEFT_SHIFT),
            ("lctrl", Key::KEY_LEFT_CONTROL),
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
        assert_eq!(key_from_str("return"), Some(Key::KEY_ENTER));
        // "esc" is an alias for "escape"
        assert_eq!(key_from_str("esc"), Some(Key::KEY_ESCAPE));
        // "shift" is an alias for "lshift"
        assert_eq!(key_from_str("shift"), Some(Key::KEY_LEFT_SHIFT));
        // "ctrl" is an alias for "lctrl"
        assert_eq!(key_from_str("ctrl"), Some(Key::KEY_LEFT_CONTROL));
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
            Some(InputBinding::Keyboard(Key::KEY_SPACE))
        );
        assert_eq!(
            binding_from_str("w"),
            Some(InputBinding::Keyboard(Key::KEY_W))
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
            binding_to_str(InputBinding::Keyboard(Key::KEY_SPACE)),
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
