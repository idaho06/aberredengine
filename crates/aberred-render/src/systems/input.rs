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
/// zero-maintenance if raylib ever adds keys.
///
/// Must run on the thread that owns the raylib window. Touches no ECS state.
fn sample_raw_device_snapshot(
    rl: &raylib::RaylibHandle,
    window_w: i32,
    window_h: i32,
) -> RawDeviceSnapshot {
    let mut raw = RawDeviceSnapshot {
        window_w,
        window_h,
        ..Default::default()
    };

    for code in 0..=MAX_KEY_CODE {
        // SAFETY: IsKeyDown reads raylib's in-memory key-state array; no
        // preconditions beyond an initialized window, which the render
        // thread guarantees.
        if unsafe { raylib::ffi::IsKeyDown(code as i32) } {
            raw.set_key(code);
        }
    }

    for button in 0..MOUSE_BUTTON_COUNT {
        // SAFETY: same as IsKeyDown above.
        if unsafe { raylib::ffi::IsMouseButtonDown(button as i32) } {
            raw.set_mouse_button(button);
        }
    }

    raw.scroll_y = rl.get_mouse_wheel_move();
    let mouse_pos = rl.get_mouse_position();
    raw.mouse_x = mouse_pos.x;
    raw.mouse_y = mouse_pos.y;

    for pad in 0..MAX_GAMEPADS as i32 {
        // SAFETY: same as IsKeyDown above -- reads raylib's in-memory
        // gamepad-state array, requires only an initialized window.
        let connected = unsafe { raylib::ffi::IsGamepadAvailable(pad) };
        let slot = &mut raw.gamepads[pad as usize];
        slot.connected = connected;
        if !connected {
            // Leave the slot at its Default::default() (all-zero) value --
            // a disconnected pad reads as all-zero, never stale, since this
            // snapshot is rebuilt fresh every frame with no carry-forward.
            continue;
        }
        for button in 1..=MAX_GAMEPAD_BUTTON {
            // SAFETY: same as IsGamepadAvailable above.
            if unsafe { raylib::ffi::IsGamepadButtonDown(pad, button) } {
                slot.set_button(button as u32);
            }
        }
        for axis in 0..GAMEPAD_AXIS_COUNT {
            // SAFETY: same as IsGamepadAvailable above.
            slot.axes[axis as usize] = unsafe { raylib::ffi::GetGamepadAxisMovement(pad, axis) };
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
