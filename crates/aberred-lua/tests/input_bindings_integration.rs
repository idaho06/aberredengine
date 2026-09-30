//! Integration tests for the Lua input-rebinding bridge.
//!
//! Covers `action_from_str` (Lua action names) and `process_input_command`
//! (Lua-facing `InputCmd` applied to `InputBindings`), including unknown
//! action/key strings being ignored rather than panicking. `InputBindings`
//! itself (defaults, rebind/add, key parsing) is unit-tested in
//! `aberred-core`'s `resources/input_bindings.rs`.

use aberred_core::events::input::InputAction;
use aberred_core::resources::input_bindings::{InputBinding, InputBindings, Key, MouseButton};

use aberred_lua::resources::lua_runtime::{InputCmd, action_from_str};
use aberred_lua::systems::lua_commands::process_input_command;

// ---------------------------------------------------------------------------
// action_from_str (Lua bridge helper)
// ---------------------------------------------------------------------------

#[test]
fn test_action_from_str_all_valid_names() {
    let pairs: &[(&str, InputAction)] = &[
        ("up", InputAction::MainDirectionUp),
        ("down", InputAction::MainDirectionDown),
        ("left", InputAction::MainDirectionLeft),
        ("right", InputAction::MainDirectionRight),
        ("main_up", InputAction::MainDirectionUp),
        ("main_down", InputAction::MainDirectionDown),
        ("main_left", InputAction::MainDirectionLeft),
        ("main_right", InputAction::MainDirectionRight),
        ("secondary_up", InputAction::SecondaryDirectionUp),
        ("secondary_down", InputAction::SecondaryDirectionDown),
        ("secondary_left", InputAction::SecondaryDirectionLeft),
        ("secondary_right", InputAction::SecondaryDirectionRight),
        ("back", InputAction::Back),
        ("action_1", InputAction::Action1),
        ("action_2", InputAction::Action2),
        ("action_3", InputAction::Action3),
        ("special", InputAction::Special),
        ("toggle_debug", InputAction::ToggleDebug),
        ("toggle_fullscreen", InputAction::ToggleFullscreen),
    ];
    for (s, expected) in pairs {
        assert_eq!(
            action_from_str(s),
            Some(*expected),
            "action_from_str({:?}) mismatch",
            s
        );
    }
}

#[test]
fn test_action_from_str_unknown_returns_none() {
    assert!(action_from_str("not_an_action").is_none());
    assert!(action_from_str("").is_none());
}

// ---------------------------------------------------------------------------
// process_input_command – Rebind
// ---------------------------------------------------------------------------

#[test]
fn test_process_input_cmd_rebind_updates_binding() {
    let mut bindings = InputBindings::default();

    process_input_command(
        InputCmd::Rebind {
            action: "action_1".to_string(),
            key: "z".to_string(),
        },
        &mut bindings,
    );

    let keys = bindings.get_bindings(InputAction::Action1);
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0], InputBinding::Keyboard(Key::KEY_Z));
}

#[test]
fn test_process_input_cmd_add_binding_appends() {
    let mut bindings = InputBindings::default();
    let initial = bindings.get_bindings(InputAction::Action2).len();

    process_input_command(
        InputCmd::AddBinding {
            action: "action_2".to_string(),
            key: "x".to_string(),
        },
        &mut bindings,
    );

    assert_eq!(
        bindings.get_bindings(InputAction::Action2).len(),
        initial + 1
    );
}

// ---------------------------------------------------------------------------
// process_input_command – unknown action / key are silently dropped
// ---------------------------------------------------------------------------

#[test]
fn test_process_input_cmd_unknown_action_does_not_panic() {
    let mut bindings = InputBindings::default();
    let snapshot = bindings.map.clone();

    // Must not panic; bindings must be unchanged
    process_input_command(
        InputCmd::Rebind {
            action: "not_a_real_action".to_string(),
            key: "a".to_string(),
        },
        &mut bindings,
    );

    assert_eq!(bindings.map, snapshot, "bindings must be unchanged");
}

#[test]
fn test_process_input_cmd_unknown_key_does_not_panic() {
    let mut bindings = InputBindings::default();

    let before = bindings.get_bindings(InputAction::Action1).to_vec();

    // Must not panic; action_1's binding must be unchanged
    process_input_command(
        InputCmd::Rebind {
            action: "action_1".to_string(),
            key: "not_a_real_key".to_string(),
        },
        &mut bindings,
    );

    assert_eq!(
        bindings.get_bindings(InputAction::Action1),
        before.as_slice()
    );
}

// ---------------------------------------------------------------------------
// Mouse button bindings
// ---------------------------------------------------------------------------

#[test]
fn test_process_input_cmd_rebind_to_mouse_button() {
    let mut bindings = InputBindings::default();

    process_input_command(
        InputCmd::Rebind {
            action: "action_3".to_string(),
            key: "mouse_left".to_string(),
        },
        &mut bindings,
    );

    let bl = bindings.get_bindings(InputAction::Action3);
    assert_eq!(bl.len(), 1);
    assert_eq!(
        bl[0],
        InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)
    );
}

#[test]
fn test_process_input_cmd_add_mouse_binding() {
    let mut bindings = InputBindings::default();
    let initial = bindings.get_bindings(InputAction::Action1).len();

    process_input_command(
        InputCmd::AddBinding {
            action: "action_1".to_string(),
            key: "mouse_middle".to_string(),
        },
        &mut bindings,
    );

    assert_eq!(
        bindings.get_bindings(InputAction::Action1).len(),
        initial + 1
    );
    assert!(
        bindings
            .get_bindings(InputAction::Action1)
            .contains(&InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE))
    );
}
