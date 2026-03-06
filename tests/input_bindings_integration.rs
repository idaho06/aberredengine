//! Integration tests for the `InputBindings` resource and input rebinding pipeline.
//!
//! These tests verify:
//! - Default bindings cover all `InputAction` variants.
//! - `rebind` and `add_binding` mutations are reflected in the resource.
//! - Lua-facing `InputCmd` commands plumbed through `process_input_command`.
//! - `BoolState` no longer carries a `key_binding` field.
//! - `InputBindings` can be inserted into a Bevy ECS `World` and read back.
//! - `key_from_str` / `key_to_str` round-trips for all documented keys.
//! - Unknown action/key strings in commands are silently ignored, not panicked.

use aberredengine::events::input::InputAction;
use aberredengine::resources::input_bindings::{
    InputBinding, InputBindings, binding_from_str, key_from_str, key_to_str,
};
use bevy_ecs::prelude::*;
use raylib::ffi::{KeyboardKey, MouseButton};

#[cfg(feature = "lua")]
use aberredengine::resources::lua_runtime::{InputCmd, action_from_str};
#[cfg(feature = "lua")]
use aberredengine::systems::lua_commands::process_input_command;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// All 15 InputAction discriminants
fn all_actions() -> Vec<InputAction> {
    vec![
        InputAction::MainDirectionUp,
        InputAction::MainDirectionDown,
        InputAction::MainDirectionLeft,
        InputAction::MainDirectionRight,
        InputAction::SecondaryDirectionUp,
        InputAction::SecondaryDirectionDown,
        InputAction::SecondaryDirectionLeft,
        InputAction::SecondaryDirectionRight,
        InputAction::Back,
        InputAction::Action1,
        InputAction::Action2,
        InputAction::Action3,
        InputAction::Special,
        InputAction::ToggleDebug,
        InputAction::ToggleFullscreen,
    ]
}

// ---------------------------------------------------------------------------
// Default bindings
// ---------------------------------------------------------------------------

/// Every InputAction must have at least one default binding so the game is
/// immediately playable without any configuration.
#[test]
fn test_default_bindings_cover_all_actions() {
    let bindings = InputBindings::default();
    for action in all_actions() {
        assert!(
            !bindings.get_bindings(action).is_empty(),
            "No default binding for {:?}",
            action
        );
    }
}

/// The default binding count should be exactly 15 (one per action).
#[test]
fn test_default_bindings_count() {
    let bindings = InputBindings::default();
    assert_eq!(
        bindings.map.len(),
        15,
        "Expected 15 default action bindings"
    );
}

// ---------------------------------------------------------------------------
// rebind / add_binding
// ---------------------------------------------------------------------------

#[test]
fn test_rebind_replaces_all_existing_bindings() {
    let mut bindings = InputBindings::default();
    // Pre-condition: Action1 has a binding
    assert!(!bindings.get_bindings(InputAction::Action1).is_empty());

    bindings.rebind(
        InputAction::Action1,
        InputBinding::Keyboard(KeyboardKey::KEY_Z),
    );

    let keys = bindings.get_bindings(InputAction::Action1);
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0], InputBinding::Keyboard(KeyboardKey::KEY_Z));
}

#[test]
fn test_add_binding_appends_without_removing_existing() {
    let mut bindings = InputBindings::default();
    let initial_count = bindings.get_bindings(InputAction::Action2).len();

    bindings.add_binding(
        InputAction::Action2,
        InputBinding::Keyboard(KeyboardKey::KEY_X),
    );

    let count_after = bindings.get_bindings(InputAction::Action2).len();
    assert_eq!(count_after, initial_count + 1);
}

#[test]
fn test_first_binding_str_returns_some_for_bound_action() {
    let bindings = InputBindings::default();
    assert!(
        bindings
            .first_binding_str(InputAction::MainDirectionUp)
            .is_some(),
        "Expected a string for bound action"
    );
}

#[test]
fn test_get_bindings_returns_empty_for_unbound_action() {
    let mut bindings = InputBindings::default();
    // Remove all bindings for Back explicitly
    bindings.map.remove(&InputAction::Back);
    assert!(bindings.get_bindings(InputAction::Back).is_empty());
}

// ---------------------------------------------------------------------------
// key_from_str / key_to_str round-trips
// ---------------------------------------------------------------------------

/// The canonical key names documented in input_bindings.rs should survive
/// a parse → serialize round-trip.
#[test]
fn test_key_str_round_trip() {
    let cases = [
        "space",
        "enter",
        "escape",
        "backspace",
        "tab",
        "up",
        "down",
        "left",
        "right",
        "a",
        "b",
        "c",
        "d",
        "e",
        "f",
        "g",
        "h",
        "i",
        "j",
        "k",
        "l",
        "m",
        "n",
        "o",
        "p",
        "q",
        "r",
        "s",
        "t",
        "u",
        "v",
        "w",
        "x",
        "y",
        "z",
        "0",
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
        "7",
        "8",
        "9",
        "f1",
        "f2",
        "f3",
        "f4",
        "f5",
        "f6",
        "f7",
        "f8",
        "f9",
        "f10",
        "f11",
        "f12",
        "lshift",
        "rshift",
        "lctrl",
        "rctrl",
        "lalt",
        "ralt",
    ];
    for name in &cases {
        let key = key_from_str(name);
        assert!(key.is_some(), "key_from_str({:?}) returned None", name);
        let back = key_to_str(key.unwrap());
        assert_eq!(back, *name, "key_to_str round-trip failed for {:?}", name);
    }
}

#[test]
fn test_key_from_str_unknown_returns_none() {
    assert!(key_from_str("not_a_real_key_xyz").is_none());
    assert!(key_from_str("").is_none());
}

// ---------------------------------------------------------------------------
// action_from_str (Lua bridge helper)
// ---------------------------------------------------------------------------

#[cfg(feature = "lua")]
#[test]
fn test_action_from_str_all_valid_names() {
    let pairs: &[(&str, InputAction)] = &[
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

#[cfg(feature = "lua")]
#[test]
fn test_action_from_str_unknown_returns_none() {
    assert!(action_from_str("not_an_action").is_none());
    assert!(action_from_str("").is_none());
}

// ---------------------------------------------------------------------------
// process_input_command – Rebind
// ---------------------------------------------------------------------------

#[cfg(feature = "lua")]
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
    assert_eq!(keys[0], InputBinding::Keyboard(KeyboardKey::KEY_Z));
}

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

    assert_eq!(bindings.map.len(), snapshot.len());
}

#[cfg(feature = "lua")]
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
// Bevy ECS resource integration
// ---------------------------------------------------------------------------

/// InputBindings must implement Resource and survive an ECS round-trip.
#[test]
fn test_input_bindings_is_ecs_resource() {
    let mut world = World::new();
    world.insert_resource(InputBindings::default());

    let res = world.get_resource::<InputBindings>();
    assert!(res.is_some(), "InputBindings not found as ECS resource");

    let bindings = res.unwrap();
    // Verify defaults are intact after the ECS round-trip
    assert!(
        !bindings
            .get_bindings(InputAction::MainDirectionUp)
            .is_empty()
    );
}

#[test]
fn test_input_bindings_mutation_via_ecs_system_state() {
    use bevy_ecs::system::SystemState;

    let mut world = World::new();
    world.insert_resource(InputBindings::default());

    // Simulate a 1-frame system that rebinds Action1 to KEY_Z
    let mut state: SystemState<ResMut<InputBindings>> = SystemState::new(&mut world);
    {
        let mut bindings = state.get_mut(&mut world);
        bindings.rebind(
            InputAction::Action1,
            InputBinding::Keyboard(KeyboardKey::KEY_Z),
        );
    }
    state.apply(&mut world);

    let bindings = world.get_resource::<InputBindings>().unwrap();
    let keys = bindings.get_bindings(InputAction::Action1);
    assert_eq!(keys, &[InputBinding::Keyboard(KeyboardKey::KEY_Z)]);
}

// ---------------------------------------------------------------------------
// BoolState no longer carries key_binding (compile-time proof)
// ---------------------------------------------------------------------------

/// This test constructs `BoolState` via `Default` to confirm the struct can be
/// created without specifying a `key_binding` field.  If `key_binding` were
/// still present, `BoolState::default()` would not compile (it didn't derive
/// `Default` before the refactor).
#[test]
fn test_bool_state_derives_default_without_key_binding() {
    use aberredengine::resources::input::BoolState;
    let state: BoolState = BoolState::default();
    // All sub-fields should be false/zero
    assert!(!state.active);
    assert!(!state.just_pressed);
    assert!(!state.just_released);
}

// ---------------------------------------------------------------------------
// Mouse button bindings
// ---------------------------------------------------------------------------

#[test]
fn test_action3_default_is_mouse_middle() {
    let bindings = InputBindings::default();
    let bl = bindings.get_bindings(InputAction::Action3);
    assert_eq!(
        bl,
        &[InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_MIDDLE)]
    );
}

#[test]
fn test_action1_default_includes_mouse_left() {
    let bindings = InputBindings::default();
    let bl = bindings.get_bindings(InputAction::Action1);
    assert!(bl.contains(&InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT)));
}

#[test]
fn test_action2_default_includes_mouse_right() {
    let bindings = InputBindings::default();
    let bl = bindings.get_bindings(InputAction::Action2);
    assert!(bl.contains(&InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_RIGHT)));
}

#[cfg(feature = "lua")]
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

#[cfg(feature = "lua")]
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

#[test]
fn test_binding_from_str_in_integration() {
    assert_eq!(
        binding_from_str("mouse_left"),
        Some(InputBinding::MouseButton(MouseButton::MOUSE_BUTTON_LEFT))
    );
    assert_eq!(
        binding_from_str("space"),
        Some(InputBinding::Keyboard(KeyboardKey::KEY_SPACE))
    );
    assert_eq!(binding_from_str("not_a_binding"), None);
}
