//! Samples raw device input each render frame and ships it to the logic thread.

use bevy_ecs::prelude::*;

use aberred_core::protocol::endpoints::LogicBridge;
use aberred_core::protocol::raw_input::{InputSample, MAX_GAMEPADS, RawDeviceSnapshot};
use crate::resources::pending_imgui_capture::PendingImguiCapture;
use crate::resources::quit_requested::QuitRequested;
use aberred_core::resources::windowsize::WindowSize;

/// Highest raylib keyboard key code in use today (`KEY_KB_MENU`).
const MAX_KEY_CODE: u32 = 348;

/// Number of raylib mouse button codes (`MOUSE_BUTTON_LEFT..=MOUSE_BUTTON_BACK`).
const MOUSE_BUTTON_COUNT: u8 = 7;

/// Highest raylib `GamepadButton` ordinal (`GAMEPAD_BUTTON_RIGHT_THUMB`); 0
/// is `GAMEPAD_BUTTON_UNKNOWN` and is skipped when polling.
const MAX_GAMEPAD_BUTTON: i32 = 17;

/// Number of raylib `GamepadAxis` values (`LEFT_X..=RIGHT_TRIGGER`).
const GAMEPAD_AXIS_COUNT: i32 = 6;

/// Poll raylib's whole keyboard + mouse held state into a
/// [`RawDeviceSnapshot`]. NO `InputBindings` resolution and NO
/// `just_pressed`/`just_released` edges -- both are computed sim-side by
/// `resolve_input_backlog` (`crate::systems::input`).
///
/// Iterates every raw key code `0..=348` and mouse button code `0..=6`
/// directly via raylib's FFI (`IsKeyDown`/`IsMouseButtonDown` take a bare
/// `int`, not the `KeyboardKey`/`MouseButton` enum) rather than a
/// hand-maintained key table -- ~355 cheap FFI calls/frame, negligible, and
/// zero-maintenance if raylib ever adds keys. The polling loops live in
/// [`fill_snapshot`]; [`RaylibDevice`] is the FFI side.
///
/// Must run on the thread that owns the raylib window. Touches no ECS state.
fn sample_raw_device_snapshot(
    rl: &raylib::RaylibHandle,
    window_w: i32,
    window_h: i32,
) -> RawDeviceSnapshot {
    fill_snapshot(&RaylibDevice(rl), window_w, window_h)
}

/// The raw device queries [`fill_snapshot`] makes, so its polling loops can
/// run against a fake device in tests.
trait RawDevice {
    fn key_down(&self, code: i32) -> bool;
    fn mouse_button_down(&self, button: i32) -> bool;
    fn mouse_wheel(&self) -> f32;
    fn mouse_position(&self) -> (f32, f32);
    fn gamepad_available(&self, pad: i32) -> bool;
    fn gamepad_button_down(&self, pad: i32, button: i32) -> bool;
    fn gamepad_axis(&self, pad: i32, axis: i32) -> f32;
}

/// raylib's live input state; one FFI/handle call per query.
struct RaylibDevice<'a>(&'a raylib::RaylibHandle);

// SAFETY (every FFI call below): these read raylib's in-memory key/mouse/
// gamepad state arrays; no preconditions beyond an initialized window, which
// holding a `RaylibHandle` on the render thread guarantees.
impl RawDevice for RaylibDevice<'_> {
    fn key_down(&self, code: i32) -> bool {
        unsafe { raylib::ffi::IsKeyDown(code) }
    }
    fn mouse_button_down(&self, button: i32) -> bool {
        unsafe { raylib::ffi::IsMouseButtonDown(button) }
    }
    fn mouse_wheel(&self) -> f32 {
        self.0.get_mouse_wheel_move()
    }
    fn mouse_position(&self) -> (f32, f32) {
        let pos = self.0.get_mouse_position();
        (pos.x, pos.y)
    }
    fn gamepad_available(&self, pad: i32) -> bool {
        unsafe { raylib::ffi::IsGamepadAvailable(pad) }
    }
    fn gamepad_button_down(&self, pad: i32, button: i32) -> bool {
        unsafe { raylib::ffi::IsGamepadButtonDown(pad, button) }
    }
    fn gamepad_axis(&self, pad: i32, axis: i32) -> f32 {
        unsafe { raylib::ffi::GetGamepadAxisMovement(pad, axis) }
    }
}

/// Build one frame's [`RawDeviceSnapshot`] by polling every key code
/// `0..=MAX_KEY_CODE`, mouse button `0..MOUSE_BUTTON_COUNT`, and, per
/// connected pad, buttons `1..=MAX_GAMEPAD_BUTTON` plus every axis.
fn fill_snapshot(device: &impl RawDevice, window_w: i32, window_h: i32) -> RawDeviceSnapshot {
    let mut raw = RawDeviceSnapshot {
        window_w,
        window_h,
        ..Default::default()
    };

    for code in 0..=MAX_KEY_CODE {
        if device.key_down(code as i32) {
            raw.set_key(code);
        }
    }

    for button in 0..MOUSE_BUTTON_COUNT {
        if device.mouse_button_down(button as i32) {
            raw.set_mouse_button(button);
        }
    }

    raw.scroll_y = device.mouse_wheel();
    (raw.mouse_x, raw.mouse_y) = device.mouse_position();

    for pad in 0..MAX_GAMEPADS as i32 {
        let connected = device.gamepad_available(pad);
        let slot = &mut raw.gamepads[pad as usize];
        slot.connected = connected;
        if !connected {
            // Leave the slot at its Default::default() (all-zero) value --
            // a disconnected pad reads as all-zero, never stale, since this
            // snapshot is rebuilt fresh every frame with no carry-forward.
            continue;
        }
        for button in 1..=MAX_GAMEPAD_BUTTON {
            if device.gamepad_button_down(pad, button) {
                slot.set_button(button as u32);
            }
        }
        for axis in 0..GAMEPAD_AXIS_COUNT {
            slot.axes[axis as usize] = device.gamepad_axis(pad, axis);
        }
    }

    raw
}

/// Samples the raw device state once per render frame (the only system
/// touching the raylib handle for input -- no bindings, no edges here, the
/// sim thread resolves all of that) and ships it (+ the previous frame's
/// imgui capture state, one-frame lag via `PendingImguiCapture`) to the
/// logic thread over the dedicated bounded input channel. A momentarily
/// full queue (sim stalled) drops the OLDEST queued sample to make room
/// (`pacing::send_or_drop_oldest`) rather than growing an unbounded backlog
/// or discarding the newest sample -- the sim resumes seeing the freshest
/// input, not stale taps from before the stall; a disconnected channel
/// (logic thread gone) sets `QuitRequested`.
pub fn sample_and_send_input(
    rl: NonSend<raylib::RaylibHandle>,
    window_size: Res<WindowSize>,
    capture: Res<PendingImguiCapture>,
    bridge: Res<LogicBridge>,
    mut quit: ResMut<QuitRequested>,
    mut prev_gamepad_connected: Local<[bool; MAX_GAMEPADS]>,
) {
    let raw = sample_raw_device_snapshot(&rl, window_size.w, window_size.h);

    for (pad, gamepad) in raw.gamepads.iter().enumerate() {
        if gamepad.connected != prev_gamepad_connected[pad] {
            log::info!(
                "Gamepad {pad}: {}",
                if gamepad.connected {
                    "connected"
                } else {
                    "disconnected"
                }
            );
            prev_gamepad_connected[pad] = gamepad.connected;
        }
    }

    let result = aberred_core::pacing::send_or_drop_oldest(
        &bridge.tx_input,
        &bridge.rx_input,
        InputSample {
            raw,
            capture: capture.0,
        },
    );
    if aberred_core::pacing::send_channel_disconnected(&result) {
        log::error!("Logic thread disconnected; shutting down");
        quit.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::protocol::raw_input::RawGamepad;
    use rustc_hash::FxHashSet;

    /// Device whose held keys/buttons include out-of-range codes too, so a
    /// test can tell a skipped code from an unpolled one.
    #[derive(Default)]
    struct FakeDevice {
        keys: FxHashSet<i32>,
        mouse: FxHashSet<i32>,
        connected: FxHashSet<i32>,
        pad_buttons: FxHashSet<(i32, i32)>,
    }

    impl RawDevice for FakeDevice {
        fn key_down(&self, code: i32) -> bool {
            self.keys.contains(&code)
        }
        fn mouse_button_down(&self, button: i32) -> bool {
            self.mouse.contains(&button)
        }
        fn mouse_wheel(&self) -> f32 {
            -1.5
        }
        fn mouse_position(&self) -> (f32, f32) {
            (12.0, 34.0)
        }
        fn gamepad_available(&self, pad: i32) -> bool {
            self.connected.contains(&pad)
        }
        fn gamepad_button_down(&self, pad: i32, button: i32) -> bool {
            self.pad_buttons.contains(&(pad, button))
        }
        fn gamepad_axis(&self, pad: i32, axis: i32) -> f32 {
            (pad * 10 + axis) as f32 / 100.0
        }
    }

    #[test]
    fn keys_and_mouse_buttons_are_polled_over_the_full_raylib_range_only() {
        let device = FakeDevice {
            keys: [0, 63, 64, 348, 349].into_iter().collect(),
            mouse: [0, 6, 7].into_iter().collect(),
            ..Default::default()
        };

        let raw = fill_snapshot(&device, 800, 600);

        for code in [0, 63, 64, 348] {
            assert!(raw.is_key_down(code), "key {code}");
        }
        assert!(!raw.is_key_down(349), "beyond KEY_KB_MENU");
        assert!(!raw.is_key_down(1));
        assert!(raw.is_mouse_button_down(0) && raw.is_mouse_button_down(6));
        assert!(!raw.is_mouse_button_down(7), "beyond MOUSE_BUTTON_BACK");
        assert_eq!((raw.window_w, raw.window_h), (800, 600));
        assert_eq!((raw.mouse_x, raw.mouse_y, raw.scroll_y), (12.0, 34.0, -1.5));
    }

    #[test]
    fn connected_pads_report_buttons_1_to_17_and_all_axes_disconnected_read_zero() {
        let device = FakeDevice {
            connected: [0].into_iter().collect(),
            pad_buttons: [(0, 0), (0, 1), (0, 17), (0, 18), (1, 1)].into_iter().collect(),
            ..Default::default()
        };

        let raw = fill_snapshot(&device, 0, 0);

        let pad = raw.gamepads[0];
        assert!(pad.connected);
        assert!(pad.is_button_down(1) && pad.is_button_down(17));
        assert!(!pad.is_button_down(0), "GAMEPAD_BUTTON_UNKNOWN is skipped");
        assert!(!pad.is_button_down(18), "beyond GAMEPAD_BUTTON_RIGHT_THUMB");
        assert_eq!(pad.axes, [0.0, 0.01, 0.02, 0.03, 0.04, 0.05]);
        for disconnected in &raw.gamepads[1..] {
            assert_eq!(*disconnected, RawGamepad::default());
        }
    }

    #[test]
    fn polling_ranges_match_raylibs_enums() {
        use raylib::ffi;
        assert_eq!(MAX_KEY_CODE, ffi::KeyboardKey::KEY_KB_MENU as u32);
        assert_eq!(MOUSE_BUTTON_COUNT, ffi::MouseButton::MOUSE_BUTTON_BACK as u8 + 1);
        assert_eq!(
            MAX_GAMEPAD_BUTTON,
            ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB as i32
        );
        assert_eq!(
            GAMEPAD_AXIS_COUNT,
            ffi::GamepadAxis::GAMEPAD_AXIS_RIGHT_TRIGGER as i32 + 1
        );
    }
}
