//! Raw, unresolved device input sampled by the render thread every frame.
//!
//! [`RawDeviceSnapshot`] carries only *physically-down* state — no
//! [`InputBindings`](crate::resources::input_bindings::InputBindings)
//! resolution, no `just_pressed`/`just_released` edges. Both are computed
//! sim-side by diffing consecutive snapshots
//! (`crate::systems::input::resolve_input_sample`), which is what lets the
//! render thread stay unaware of bindings entirely.
//!
//! [`InputSample`] is the payload shipped over the dedicated bounded input
//! channel (`LogicBridge::tx_input` / `LogicInit::rx_input`) — a raw snapshot
//! plus the imgui capture state that rides alongside it, one render-frame
//! stale.

/// Whether the debug imgui overlay currently wants to capture mouse/keyboard
/// input this frame. Read from the render thread after `ImguiBridge::render`
/// runs, carried one frame across the thread boundary via
/// `InputSample::capture` (same latency class as `SignalIntents`, riding the
/// dedicated bounded input channel), and used by `resolve_input_backlog` to
/// mask gameplay input while the debug panel has focus. Scoped to the F11
/// debug overlay only -- the in-house `GuiButton`/`GuiWindow` system does its
/// own hit-testing and isn't imgui.
///
/// Lives here (a plain POD wire type, `protocol/`) rather than under
/// `resources::render` -- it names no imgui type and crosses the
/// logic/render thread boundary every frame, matching every other
/// cross-thread bridge/protocol type's home.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImguiCaptureState {
    pub mouse: bool,
    pub keyboard: bool,
}

/// Number of `u64` words in the keyboard bitset. `8 * 64 = 512` bits,
/// comfortably covering every raylib keycode (`KEY_KB_MENU = 348` is the
/// highest defined today).
const KEY_WORDS: usize = 8;

/// Maximum simultaneously-tracked gamepads (matches raylib's own internal
/// `MAX_GAMEPADS`, not exposed via the Rust FFI bindings so hardcoded here,
/// same convention as this module's key/mouse-button counts).
pub const MAX_GAMEPADS: usize = 4;

/// One render frame's raw state for a single gamepad slot.
///
/// `buttons`' bit index equals raylib's `GamepadButton` ordinal (0..=17
/// fits comfortably in a `u32`, room to spare); `axes` is indexed by
/// raylib's `GamepadAxis` ordinal (`LEFT_X`=0, `LEFT_Y`=1, `RIGHT_X`=2,
/// `RIGHT_Y`=3, `LEFT_TRIGGER`=4, `RIGHT_TRIGGER`=5) — no remapping.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawGamepad {
    pub connected: bool,
    pub buttons: u32,
    pub axes: [f32; 6],
}

impl RawGamepad {
    /// Mark gamepad `button` (a raylib `GamepadButton` ordinal) as down.
    pub fn set_button(&mut self, button: u32) {
        if button < 32 {
            self.buttons |= 1u32 << button;
        }
    }

    /// Whether gamepad `button` (a raylib `GamepadButton` ordinal) is down.
    pub fn is_button_down(&self, button: u32) -> bool {
        button < 32 && (self.buttons >> button) & 1 != 0
    }
}

/// One render frame's raw device state: which keys/mouse buttons/gamepad
/// buttons are physically down, plus mouse/gamepad-axis analog values and
/// window dimensions.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RawDeviceSnapshot {
    /// Bitset over raylib keyboard key codes. Bit `i` of word `i / 64` set
    /// means key code `i` is physically down.
    pub keys: [u64; KEY_WORDS],
    /// Bitmask over raylib `MouseButton` codes (0..=6).
    pub mouse_buttons: u8,
    /// Mouse position in WINDOW coordinates. Letterbox correction into
    /// game-space happens sim-side (`WindowSize::window_to_game_pos`), since
    /// that needs `ScreenSize`, which the sim world owns.
    pub mouse_x: f32,
    pub mouse_y: f32,
    /// Mouse wheel delta this frame (positive = up).
    pub scroll_y: f32,
    /// OS window dimensions at sample time.
    pub window_w: i32,
    pub window_h: i32,
    /// Per-pad raw gamepad state. A disconnected pad is
    /// `RawGamepad::default()` (all-zero) — the render thread rebuilds this
    /// array fresh every frame rather than carrying forward the previous
    /// frame's state, so a disconnect reads as all-zero, never stale.
    pub gamepads: [RawGamepad; MAX_GAMEPADS],
}

impl RawDeviceSnapshot {
    /// Mark keyboard `code` as down. Silently ignores codes outside the
    /// bitset's range (the extra space keeps room for additional keys).
    pub fn set_key(&mut self, code: u32) {
        let word = (code / 64) as usize;
        let bit = code % 64;
        if let Some(w) = self.keys.get_mut(word) {
            *w |= 1u64 << bit;
        }
    }

    /// Whether keyboard `code` is down in this snapshot.
    pub fn is_key_down(&self, code: u32) -> bool {
        let word = (code / 64) as usize;
        let bit = code % 64;
        self.keys.get(word).is_some_and(|w| (w >> bit) & 1 != 0)
    }

    /// Mark mouse `button` (0..=6) as down.
    pub fn set_mouse_button(&mut self, button: u8) {
        if button < 8 {
            self.mouse_buttons |= 1u8 << button;
        }
    }

    /// Whether mouse `button` (0..=6) is down in this snapshot.
    pub fn is_mouse_button_down(&self, button: u8) -> bool {
        button < 8 && (self.mouse_buttons >> button) & 1 != 0
    }
}

/// One render frame's input payload, sent over the dedicated bounded input
/// channel.
#[derive(Debug, Clone, Default)]
pub struct InputSample {
    pub raw: RawDeviceSnapshot,
    /// Previous render frame's imgui capture state — see the module doc
    /// comment. Used sim-side to mask gameplay input while the F11 debug
    /// overlay has focus.
    pub capture: ImguiCaptureState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_bit_zero_round_trips() {
        let mut s = RawDeviceSnapshot::default();
        assert!(!s.is_key_down(0));
        s.set_key(0);
        assert!(s.is_key_down(0));
        assert!(!s.is_key_down(1));
    }

    #[test]
    fn key_word_boundary_63_64_round_trip() {
        let mut s = RawDeviceSnapshot::default();
        s.set_key(63);
        s.set_key(64);
        assert!(s.is_key_down(63));
        assert!(s.is_key_down(64));
        assert!(!s.is_key_down(62));
        assert!(!s.is_key_down(65));
    }

    #[test]
    fn key_kb_menu_348_round_trips() {
        // KEY_KB_MENU = 348, the highest raylib keycode in use today.
        let mut s = RawDeviceSnapshot::default();
        s.set_key(348);
        assert!(s.is_key_down(348));
        assert!(!s.is_key_down(347));
    }

    #[test]
    fn key_out_of_range_is_ignored_not_panicking() {
        let mut s = RawDeviceSnapshot::default();
        s.set_key(10_000);
        assert!(!s.is_key_down(10_000));
    }

    #[test]
    fn mouse_button_round_trips_and_out_of_range_is_ignored() {
        let mut s = RawDeviceSnapshot::default();
        s.set_mouse_button(0);
        s.set_mouse_button(6);
        assert!(s.is_mouse_button_down(0));
        assert!(s.is_mouse_button_down(6));
        assert!(!s.is_mouse_button_down(1));
        s.set_mouse_button(200);
        assert!(!s.is_mouse_button_down(200));
    }

    #[test]
    fn gamepad_button_round_trips() {
        let mut g = RawGamepad::default();
        assert!(!g.is_button_down(0));
        g.set_button(0);
        g.set_button(17);
        assert!(g.is_button_down(0));
        assert!(g.is_button_down(17));
        assert!(!g.is_button_down(1));
    }

    #[test]
    fn gamepad_button_out_of_range_is_ignored_not_panicking() {
        let mut g = RawGamepad::default();
        g.set_button(32);
        assert!(!g.is_button_down(32));
    }

    #[test]
    fn disconnected_gamepad_slot_defaults_to_all_zero() {
        let s = RawDeviceSnapshot::default();
        assert!(!s.gamepads[0].connected);
        assert_eq!(s.gamepads[0].buttons, 0);
        assert_eq!(s.gamepads[0].axes, [0.0; 6]);
    }
}
