//! Input systems.
//!
//! - [`update_input_state`] reads hardware input from Raylib each frame,
//!   looks up the current bindings from [`InputBindings`], and writes the
//!   results into [`InputState`].
//! - Input events are emitted for key presses/releases. Debug and fullscreen
//!   toggle actions additionally trigger their own events
//!   ([`SwitchDebugEvent`], [`SwitchFullScreenEvent`]).
use bevy_ecs::prelude::*;

use log::debug;

use crate::events::input::{InputAction, InputEvent};
use crate::events::switchdebug::SwitchDebugEvent;
use crate::events::switchfullscreen::SwitchFullScreenEvent;
use crate::resources::input::InputState;
use crate::resources::input_bindings::{InputBinding, InputBindings};

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn any_binding_down(rl: &raylib::RaylibHandle, bindings: &[InputBinding]) -> bool {
    bindings.iter().any(|b| match b {
        InputBinding::Keyboard(k) => rl.is_key_down(*k),
        InputBinding::MouseButton(m) => rl.is_mouse_button_down(*m),
    })
}

fn any_binding_pressed(rl: &raylib::RaylibHandle, bindings: &[InputBinding]) -> bool {
    bindings.iter().any(|b| match b {
        InputBinding::Keyboard(k) => rl.is_key_pressed(*k),
        InputBinding::MouseButton(m) => rl.is_mouse_button_pressed(*m),
    })
}

fn any_binding_released(rl: &raylib::RaylibHandle, bindings: &[InputBinding]) -> bool {
    bindings.iter().any(|b| match b {
        InputBinding::Keyboard(k) => rl.is_key_released(*k),
        InputBinding::MouseButton(m) => rl.is_mouse_button_released(*m),
    })
}

// ---------------------------------------------------------------------------
// System
// ---------------------------------------------------------------------------

/// Poll Raylib for keyboard input and update the `InputState` resource.
///
/// This system is the single source of truth for input polling.  It looks up
/// each action's hardware bindings from [`InputBindings`] and writes the
/// resulting state into [`InputState`].  Consuming systems (movement
/// controllers, menu observer, etc.) are unchanged.
///
/// The `just_pressed` / `just_released` fields use **any-binding** semantics:
/// either is `true` when *at least one* bound key triggered that edge.
pub fn update_input_state(
    mut input: ResMut<InputState>,
    bindings: Res<InputBindings>,
    rl: NonSendMut<raylib::RaylibHandle>,
    mut commands: Commands,
) {
    // Inline macro: update one BoolState field and optionally emit an InputEvent.
    //
    // `$state`  – a field path into `input` (e.g. `input.maindirection_up`)
    // `$action` – the InputAction variant used to look up bindings
    // Variants:
    //   poll_action!($state, $action)           — updates state, emits InputEvent
    //   poll_action!(no_event; $state, $action) — updates state only (special actions)
    macro_rules! poll_action {
        ($state:expr, $action:expr) => {{
            let bl = bindings.get_bindings($action);
            $state.active = any_binding_down(&rl, bl);
            if any_binding_pressed(&rl, bl) {
                $state.just_pressed = true;
                commands.trigger(InputEvent {
                    action: $action,
                    pressed: true,
                });
            } else {
                $state.just_pressed = false;
            }
            if any_binding_released(&rl, bl) {
                $state.just_released = true;
                commands.trigger(InputEvent {
                    action: $action,
                    pressed: false,
                });
            } else {
                $state.just_released = false;
            }
        }};
        (no_event; $state:expr, $action:expr) => {{
            let bl = bindings.get_bindings($action);
            $state.active = any_binding_down(&rl, bl);
            if any_binding_pressed(&rl, bl) {
                $state.just_pressed = true;
            } else {
                $state.just_pressed = false;
            }
            if any_binding_released(&rl, bl) {
                $state.just_released = true;
            } else {
                $state.just_released = false;
            }
        }};
    }

    // --- Primary direction (WASD) ---
    poll_action!(input.maindirection_up, InputAction::MainDirectionUp);
    poll_action!(input.maindirection_down, InputAction::MainDirectionDown);
    poll_action!(input.maindirection_left, InputAction::MainDirectionLeft);
    poll_action!(input.maindirection_right, InputAction::MainDirectionRight);

    // --- Secondary direction (arrow keys) ---
    poll_action!(
        input.secondarydirection_up,
        InputAction::SecondaryDirectionUp
    );
    poll_action!(
        input.secondarydirection_down,
        InputAction::SecondaryDirectionDown
    );
    poll_action!(
        input.secondarydirection_left,
        InputAction::SecondaryDirectionLeft
    );
    poll_action!(
        input.secondarydirection_right,
        InputAction::SecondaryDirectionRight
    );

    // --- Action buttons ---
    poll_action!(input.action_back, InputAction::Back);
    poll_action!(input.action_1, InputAction::Action1);
    poll_action!(input.action_2, InputAction::Action2);
    poll_action!(input.action_3, InputAction::Action3);
    poll_action!(input.action_special, InputAction::Special);

    // --- Special toggles ---
    // mode_debug and fullscreen_toggle don't emit InputEvent; they trigger their
    // own dedicated events so existing observers don't need to change.
    poll_action!(no_event; input.mode_debug, InputAction::ToggleDebug);
    if input.mode_debug.just_pressed {
        debug!("Debug mode key pressed");
        commands.trigger(SwitchDebugEvent {});
    }

    poll_action!(no_event; input.fullscreen_toggle, InputAction::ToggleFullscreen);
    if input.fullscreen_toggle.just_pressed {
        debug!("Fullscreen toggle key pressed");
        commands.trigger(SwitchFullScreenEvent {});
    }

    // --- Mouse wheel (analog scroll) ---
    input.scroll_y = rl.get_mouse_wheel_move();
}
