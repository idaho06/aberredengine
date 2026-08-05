//! Regression test: engine-owned `Key`/`MouseButton`/`GamepadButton`/
//! `GamepadAxis` numeric codes (`src/resources/input_bindings.rs`) must stay
//! identical to raylib's own codes. These newtypes replaced direct
//! `raylib::ffi::{KeyboardKey,MouseButton,GamepadButton,GamepadAxis}` usage in
//! core (`docs/plans/remove-raylib-from-aberred-core.md`, Phase 3) so core no
//! longer imports raylib types -- but the numeric values still have to match,
//! since `RawDeviceSnapshot`'s bitset (fed by raylib key/button/axis codes on
//! the render thread) is diffed against these consts on the logic thread.
//!
//! This file (not `src/`) is intentionally the only place in the repository
//! that imports both `raylib::ffi` and `aberredengine::resources::input_bindings`
//! side by side, so a raylib version bump that renumbers a code is caught
//! here instead of silently desyncing input resolution. Coverage is
//! exhaustive (every const on every type), not a spot-check -- hand-copying
//! raylib's numeric codes into these consts has no other backstop, so a
//! single transcription typo (most likely among the low, easy-to-swap values
//! like `KEY_BACK`/`KEY_MENU`/`KEY_VOLUME_UP`/`KEY_VOLUME_DOWN`) must fail here.

use aberredengine::core::resources::input_bindings::{GamepadAxis, GamepadButton, Key, MouseButton};
use raylib::ffi;

#[test]
fn key_codes_match_raylib() {
    let pairs = [
        (Key::KEY_NULL.0, ffi::KeyboardKey::KEY_NULL as u16),
        (Key::KEY_APOSTROPHE.0, ffi::KeyboardKey::KEY_APOSTROPHE as u16),
        (Key::KEY_COMMA.0, ffi::KeyboardKey::KEY_COMMA as u16),
        (Key::KEY_MINUS.0, ffi::KeyboardKey::KEY_MINUS as u16),
        (Key::KEY_PERIOD.0, ffi::KeyboardKey::KEY_PERIOD as u16),
        (Key::KEY_SLASH.0, ffi::KeyboardKey::KEY_SLASH as u16),
        (Key::KEY_ZERO.0, ffi::KeyboardKey::KEY_ZERO as u16),
        (Key::KEY_ONE.0, ffi::KeyboardKey::KEY_ONE as u16),
        (Key::KEY_TWO.0, ffi::KeyboardKey::KEY_TWO as u16),
        (Key::KEY_THREE.0, ffi::KeyboardKey::KEY_THREE as u16),
        (Key::KEY_FOUR.0, ffi::KeyboardKey::KEY_FOUR as u16),
        (Key::KEY_FIVE.0, ffi::KeyboardKey::KEY_FIVE as u16),
        (Key::KEY_SIX.0, ffi::KeyboardKey::KEY_SIX as u16),
        (Key::KEY_SEVEN.0, ffi::KeyboardKey::KEY_SEVEN as u16),
        (Key::KEY_EIGHT.0, ffi::KeyboardKey::KEY_EIGHT as u16),
        (Key::KEY_NINE.0, ffi::KeyboardKey::KEY_NINE as u16),
        (Key::KEY_SEMICOLON.0, ffi::KeyboardKey::KEY_SEMICOLON as u16),
        (Key::KEY_EQUAL.0, ffi::KeyboardKey::KEY_EQUAL as u16),
        (Key::KEY_A.0, ffi::KeyboardKey::KEY_A as u16),
        (Key::KEY_B.0, ffi::KeyboardKey::KEY_B as u16),
        (Key::KEY_C.0, ffi::KeyboardKey::KEY_C as u16),
        (Key::KEY_D.0, ffi::KeyboardKey::KEY_D as u16),
        (Key::KEY_E.0, ffi::KeyboardKey::KEY_E as u16),
        (Key::KEY_F.0, ffi::KeyboardKey::KEY_F as u16),
        (Key::KEY_G.0, ffi::KeyboardKey::KEY_G as u16),
        (Key::KEY_H.0, ffi::KeyboardKey::KEY_H as u16),
        (Key::KEY_I.0, ffi::KeyboardKey::KEY_I as u16),
        (Key::KEY_J.0, ffi::KeyboardKey::KEY_J as u16),
        (Key::KEY_K.0, ffi::KeyboardKey::KEY_K as u16),
        (Key::KEY_L.0, ffi::KeyboardKey::KEY_L as u16),
        (Key::KEY_M.0, ffi::KeyboardKey::KEY_M as u16),
        (Key::KEY_N.0, ffi::KeyboardKey::KEY_N as u16),
        (Key::KEY_O.0, ffi::KeyboardKey::KEY_O as u16),
        (Key::KEY_P.0, ffi::KeyboardKey::KEY_P as u16),
        (Key::KEY_Q.0, ffi::KeyboardKey::KEY_Q as u16),
        (Key::KEY_R.0, ffi::KeyboardKey::KEY_R as u16),
        (Key::KEY_S.0, ffi::KeyboardKey::KEY_S as u16),
        (Key::KEY_T.0, ffi::KeyboardKey::KEY_T as u16),
        (Key::KEY_U.0, ffi::KeyboardKey::KEY_U as u16),
        (Key::KEY_V.0, ffi::KeyboardKey::KEY_V as u16),
        (Key::KEY_W.0, ffi::KeyboardKey::KEY_W as u16),
        (Key::KEY_X.0, ffi::KeyboardKey::KEY_X as u16),
        (Key::KEY_Y.0, ffi::KeyboardKey::KEY_Y as u16),
        (Key::KEY_Z.0, ffi::KeyboardKey::KEY_Z as u16),
        (Key::KEY_LEFT_BRACKET.0, ffi::KeyboardKey::KEY_LEFT_BRACKET as u16),
        (Key::KEY_BACKSLASH.0, ffi::KeyboardKey::KEY_BACKSLASH as u16),
        (Key::KEY_RIGHT_BRACKET.0, ffi::KeyboardKey::KEY_RIGHT_BRACKET as u16),
        (Key::KEY_GRAVE.0, ffi::KeyboardKey::KEY_GRAVE as u16),
        (Key::KEY_SPACE.0, ffi::KeyboardKey::KEY_SPACE as u16),
        (Key::KEY_ESCAPE.0, ffi::KeyboardKey::KEY_ESCAPE as u16),
        (Key::KEY_ENTER.0, ffi::KeyboardKey::KEY_ENTER as u16),
        (Key::KEY_TAB.0, ffi::KeyboardKey::KEY_TAB as u16),
        (Key::KEY_BACKSPACE.0, ffi::KeyboardKey::KEY_BACKSPACE as u16),
        (Key::KEY_INSERT.0, ffi::KeyboardKey::KEY_INSERT as u16),
        (Key::KEY_DELETE.0, ffi::KeyboardKey::KEY_DELETE as u16),
        (Key::KEY_RIGHT.0, ffi::KeyboardKey::KEY_RIGHT as u16),
        (Key::KEY_LEFT.0, ffi::KeyboardKey::KEY_LEFT as u16),
        (Key::KEY_DOWN.0, ffi::KeyboardKey::KEY_DOWN as u16),
        (Key::KEY_UP.0, ffi::KeyboardKey::KEY_UP as u16),
        (Key::KEY_PAGE_UP.0, ffi::KeyboardKey::KEY_PAGE_UP as u16),
        (Key::KEY_PAGE_DOWN.0, ffi::KeyboardKey::KEY_PAGE_DOWN as u16),
        (Key::KEY_HOME.0, ffi::KeyboardKey::KEY_HOME as u16),
        (Key::KEY_END.0, ffi::KeyboardKey::KEY_END as u16),
        (Key::KEY_CAPS_LOCK.0, ffi::KeyboardKey::KEY_CAPS_LOCK as u16),
        (Key::KEY_SCROLL_LOCK.0, ffi::KeyboardKey::KEY_SCROLL_LOCK as u16),
        (Key::KEY_NUM_LOCK.0, ffi::KeyboardKey::KEY_NUM_LOCK as u16),
        (Key::KEY_PRINT_SCREEN.0, ffi::KeyboardKey::KEY_PRINT_SCREEN as u16),
        (Key::KEY_PAUSE.0, ffi::KeyboardKey::KEY_PAUSE as u16),
        (Key::KEY_F1.0, ffi::KeyboardKey::KEY_F1 as u16),
        (Key::KEY_F2.0, ffi::KeyboardKey::KEY_F2 as u16),
        (Key::KEY_F3.0, ffi::KeyboardKey::KEY_F3 as u16),
        (Key::KEY_F4.0, ffi::KeyboardKey::KEY_F4 as u16),
        (Key::KEY_F5.0, ffi::KeyboardKey::KEY_F5 as u16),
        (Key::KEY_F6.0, ffi::KeyboardKey::KEY_F6 as u16),
        (Key::KEY_F7.0, ffi::KeyboardKey::KEY_F7 as u16),
        (Key::KEY_F8.0, ffi::KeyboardKey::KEY_F8 as u16),
        (Key::KEY_F9.0, ffi::KeyboardKey::KEY_F9 as u16),
        (Key::KEY_F10.0, ffi::KeyboardKey::KEY_F10 as u16),
        (Key::KEY_F11.0, ffi::KeyboardKey::KEY_F11 as u16),
        (Key::KEY_F12.0, ffi::KeyboardKey::KEY_F12 as u16),
        (Key::KEY_LEFT_SHIFT.0, ffi::KeyboardKey::KEY_LEFT_SHIFT as u16),
        (Key::KEY_LEFT_CONTROL.0, ffi::KeyboardKey::KEY_LEFT_CONTROL as u16),
        (Key::KEY_LEFT_ALT.0, ffi::KeyboardKey::KEY_LEFT_ALT as u16),
        (Key::KEY_LEFT_SUPER.0, ffi::KeyboardKey::KEY_LEFT_SUPER as u16),
        (Key::KEY_RIGHT_SHIFT.0, ffi::KeyboardKey::KEY_RIGHT_SHIFT as u16),
        (Key::KEY_RIGHT_CONTROL.0, ffi::KeyboardKey::KEY_RIGHT_CONTROL as u16),
        (Key::KEY_RIGHT_ALT.0, ffi::KeyboardKey::KEY_RIGHT_ALT as u16),
        (Key::KEY_RIGHT_SUPER.0, ffi::KeyboardKey::KEY_RIGHT_SUPER as u16),
        (Key::KEY_KB_MENU.0, ffi::KeyboardKey::KEY_KB_MENU as u16),
        (Key::KEY_KP_0.0, ffi::KeyboardKey::KEY_KP_0 as u16),
        (Key::KEY_KP_1.0, ffi::KeyboardKey::KEY_KP_1 as u16),
        (Key::KEY_KP_2.0, ffi::KeyboardKey::KEY_KP_2 as u16),
        (Key::KEY_KP_3.0, ffi::KeyboardKey::KEY_KP_3 as u16),
        (Key::KEY_KP_4.0, ffi::KeyboardKey::KEY_KP_4 as u16),
        (Key::KEY_KP_5.0, ffi::KeyboardKey::KEY_KP_5 as u16),
        (Key::KEY_KP_6.0, ffi::KeyboardKey::KEY_KP_6 as u16),
        (Key::KEY_KP_7.0, ffi::KeyboardKey::KEY_KP_7 as u16),
        (Key::KEY_KP_8.0, ffi::KeyboardKey::KEY_KP_8 as u16),
        (Key::KEY_KP_9.0, ffi::KeyboardKey::KEY_KP_9 as u16),
        (Key::KEY_KP_DECIMAL.0, ffi::KeyboardKey::KEY_KP_DECIMAL as u16),
        (Key::KEY_KP_DIVIDE.0, ffi::KeyboardKey::KEY_KP_DIVIDE as u16),
        (Key::KEY_KP_MULTIPLY.0, ffi::KeyboardKey::KEY_KP_MULTIPLY as u16),
        (Key::KEY_KP_SUBTRACT.0, ffi::KeyboardKey::KEY_KP_SUBTRACT as u16),
        (Key::KEY_KP_ADD.0, ffi::KeyboardKey::KEY_KP_ADD as u16),
        (Key::KEY_KP_ENTER.0, ffi::KeyboardKey::KEY_KP_ENTER as u16),
        (Key::KEY_KP_EQUAL.0, ffi::KeyboardKey::KEY_KP_EQUAL as u16),
        (Key::KEY_BACK.0, ffi::KeyboardKey::KEY_BACK as u16),
        (Key::KEY_MENU.0, ffi::KeyboardKey::KEY_MENU as u16),
        (Key::KEY_VOLUME_UP.0, ffi::KeyboardKey::KEY_VOLUME_UP as u16),
        (Key::KEY_VOLUME_DOWN.0, ffi::KeyboardKey::KEY_VOLUME_DOWN as u16),
    ];
    for (engine, raylib) in pairs {
        assert_eq!(engine, raylib, "Key code mismatch against raylib::ffi::KeyboardKey");
    }
}

#[test]
fn mouse_button_codes_match_raylib() {
    let pairs = [
        (MouseButton::MOUSE_BUTTON_LEFT.0, ffi::MouseButton::MOUSE_BUTTON_LEFT as u16),
        (MouseButton::MOUSE_BUTTON_RIGHT.0, ffi::MouseButton::MOUSE_BUTTON_RIGHT as u16),
        (MouseButton::MOUSE_BUTTON_MIDDLE.0, ffi::MouseButton::MOUSE_BUTTON_MIDDLE as u16),
        (MouseButton::MOUSE_BUTTON_SIDE.0, ffi::MouseButton::MOUSE_BUTTON_SIDE as u16),
        (MouseButton::MOUSE_BUTTON_EXTRA.0, ffi::MouseButton::MOUSE_BUTTON_EXTRA as u16),
        (MouseButton::MOUSE_BUTTON_FORWARD.0, ffi::MouseButton::MOUSE_BUTTON_FORWARD as u16),
        (MouseButton::MOUSE_BUTTON_BACK.0, ffi::MouseButton::MOUSE_BUTTON_BACK as u16),
    ];
    for (engine, raylib) in pairs {
        assert_eq!(engine, raylib, "MouseButton code mismatch against raylib::ffi::MouseButton");
    }
}

#[test]
fn gamepad_button_codes_match_raylib() {
    let pairs = [
        (GamepadButton::GAMEPAD_BUTTON_UNKNOWN.0, ffi::GamepadButton::GAMEPAD_BUTTON_UNKNOWN as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_UP as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_RIGHT as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_DOWN as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_FACE_LEFT as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_UP as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_RIGHT as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_LEFT as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_1 as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_TRIGGER_2 as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_1 as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_TRIGGER_2 as u16),
        (GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT.0, ffi::GamepadButton::GAMEPAD_BUTTON_MIDDLE_LEFT as u16),
        (GamepadButton::GAMEPAD_BUTTON_MIDDLE.0, ffi::GamepadButton::GAMEPAD_BUTTON_MIDDLE as u16),
        (GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT.0, ffi::GamepadButton::GAMEPAD_BUTTON_MIDDLE_RIGHT as u16),
        (GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB.0, ffi::GamepadButton::GAMEPAD_BUTTON_LEFT_THUMB as u16),
        (GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB.0, ffi::GamepadButton::GAMEPAD_BUTTON_RIGHT_THUMB as u16),
    ];
    for (engine, raylib) in pairs {
        assert_eq!(engine, raylib, "GamepadButton code mismatch against raylib::ffi::GamepadButton");
    }
}

#[test]
fn gamepad_axis_codes_match_raylib() {
    let pairs = [
        (GamepadAxis::GAMEPAD_AXIS_LEFT_X.0, ffi::GamepadAxis::GAMEPAD_AXIS_LEFT_X as u16),
        (GamepadAxis::GAMEPAD_AXIS_LEFT_Y.0, ffi::GamepadAxis::GAMEPAD_AXIS_LEFT_Y as u16),
        (GamepadAxis::GAMEPAD_AXIS_RIGHT_X.0, ffi::GamepadAxis::GAMEPAD_AXIS_RIGHT_X as u16),
        (GamepadAxis::GAMEPAD_AXIS_RIGHT_Y.0, ffi::GamepadAxis::GAMEPAD_AXIS_RIGHT_Y as u16),
        (GamepadAxis::GAMEPAD_AXIS_LEFT_TRIGGER.0, ffi::GamepadAxis::GAMEPAD_AXIS_LEFT_TRIGGER as u16),
        (GamepadAxis::GAMEPAD_AXIS_RIGHT_TRIGGER.0, ffi::GamepadAxis::GAMEPAD_AXIS_RIGHT_TRIGGER as u16),
    ];
    for (engine, raylib) in pairs {
        assert_eq!(engine, raylib, "GamepadAxis code mismatch against raylib::ffi::GamepadAxis");
    }
}
