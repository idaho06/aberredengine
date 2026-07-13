//! Raw, unresolved device input sampled by the render thread every frame
//! (Phase 7d, `docs/plans/phase7d-raw-input-ownership.md`).
//!
//! [`RawDeviceSnapshot`] carries only *physically-down* state — no
//! [`InputBindings`](crate::resources::input_bindings::InputBindings)
//! resolution, no `just_pressed`/`just_released` edges. Both are computed
//! sim-side by diffing consecutive snapshots
//! (`crate::systems::input::resolve_input_sample`), which is what lets the
//! render thread stop caring about bindings entirely.
//!
//! [`InputSample`] is the payload shipped over the dedicated bounded input
//! channel (`LogicBridge::tx_input` / `LogicInit::rx_input`) — a raw snapshot
//! plus the imgui capture state that rides alongside it (one render-frame
//! stale, same latency class as before Phase 7d).

use crate::resources::imgui_bridge::ImguiCaptureState;

/// Number of `u64` words in the keyboard bitset. `8 * 64 = 512` bits,
/// comfortably covering every raylib keycode (`KEY_KB_MENU = 348` is the
/// highest defined today).
const KEY_WORDS: usize = 8;

/// One render frame's raw device state: which keys/mouse buttons are
/// physically down, plus mouse position/wheel and window dimensions.
///
/// Gamepad support is intentionally absent — a future addition would add
/// `axes`/`buttons`/`connected` fields here, per the Phase 7d plan.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
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
}

impl RawDeviceSnapshot {
    /// Mark keyboard `code` as down. Silently ignores codes outside the
    /// bitset's range (there are none today; this is slack for future keys).
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
}
