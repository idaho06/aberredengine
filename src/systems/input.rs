//! Input systems.
//!
//! Since Phase 5a of the render/logic thread split, input handling is split
//! into two halves along the seam the future thread boundary will cut:
//!
//! - [`sample_input_snapshot`] — a plain function (not a system) that polls
//!   raylib and resolves the current [`InputBindings`] into a fully owned
//!   [`RawInputSnapshot`]. Render-thread work: it is the only input code that
//!   touches the `RaylibHandle`. Called directly by the main loop once per
//!   render frame.
//! - [`apply_input_snapshot`] — a system that copies the latest snapshot into
//!   [`InputState`], fills in the camera-dependent `mouse_world_x/y`
//!   projection, and emits the input events ([`InputEvent`],
//!   [`SwitchDebugEvent`], [`SwitchFullScreenEvent`]) from the snapshot's
//!   edge flags. Logic-side work: no raylib handle involved.

use bevy_ecs::prelude::*;

use log::debug;

use raylib::math::Vector2;
use raylib::prelude::Camera2D;

use crate::events::input::{InputAction, InputEvent};
use crate::events::switchdebug::SwitchDebugEvent;
use crate::events::switchfullscreen::SwitchFullScreenEvent;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::input::{BoolState, InputState};
use crate::resources::input_bindings::{InputBinding, InputBindings};
use crate::resources::rawinput::{LatestInputSnapshot, RawInputSnapshot};
use crate::resources::screensize::ScreenSize;
use crate::resources::windowsize::WindowSize;

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

/// Project a game-space (render-target) position into world space through a
/// 2D camera — the handle-free equivalent of
/// `RaylibHandle::get_screen_to_world2D`.
///
/// SAFETY: `GetScreenToWorld2D` is pure matrix math (camera matrix +
/// inverse, rcore.c); it reads no window/GL state and is safe to call
/// without `InitWindow`, which is what lets the logic side compute the
/// world-space mouse without a `RaylibHandle`.
pub fn screen_to_world2d(position: Vector2, camera: &Camera2D) -> Vector2 {
    unsafe { raylib::ffi::GetScreenToWorld2D(position.into(), (*camera).into()).into() }
}

// ---------------------------------------------------------------------------
// Render-side sampling (plain function, not a system)
// ---------------------------------------------------------------------------

/// Poll raylib and resolve [`InputBindings`] into a [`RawInputSnapshot`].
///
/// The single source of truth for input polling. `just_pressed` /
/// `just_released` use **any-binding** semantics: either is `true` when *at
/// least one* bound key triggered that edge. `mouse_x`/`mouse_y` are
/// letterbox-corrected game-space coordinates; `mouse_world_x/y` are left
/// `0.0` (camera-dependent — filled in by [`apply_input_snapshot`]).
///
/// Must run on the thread that owns the raylib window. Emits no events and
/// touches no ECS state — the event edges are derived from the snapshot by
/// [`apply_input_snapshot`].
pub fn sample_input_snapshot(
    rl: &raylib::RaylibHandle,
    bindings: &InputBindings,
    window_size: &WindowSize,
    screen_size: &ScreenSize,
) -> RawInputSnapshot {
    let mut state = InputState::default();

    // Inline macro: sample one BoolState field from its action's bindings.
    macro_rules! sample_action {
        ($state:expr, $action:expr) => {{
            let bl = bindings.get_bindings($action);
            $state.active = any_binding_down(rl, bl);
            $state.just_pressed = any_binding_pressed(rl, bl);
            $state.just_released = any_binding_released(rl, bl);
        }};
    }

    // --- Primary direction (WASD) ---
    sample_action!(state.maindirection_up, InputAction::MainDirectionUp);
    sample_action!(state.maindirection_down, InputAction::MainDirectionDown);
    sample_action!(state.maindirection_left, InputAction::MainDirectionLeft);
    sample_action!(state.maindirection_right, InputAction::MainDirectionRight);

    // --- Secondary direction (arrow keys) ---
    sample_action!(
        state.secondarydirection_up,
        InputAction::SecondaryDirectionUp
    );
    sample_action!(
        state.secondarydirection_down,
        InputAction::SecondaryDirectionDown
    );
    sample_action!(
        state.secondarydirection_left,
        InputAction::SecondaryDirectionLeft
    );
    sample_action!(
        state.secondarydirection_right,
        InputAction::SecondaryDirectionRight
    );

    // --- Action buttons ---
    sample_action!(state.action_back, InputAction::Back);
    sample_action!(state.action_1, InputAction::Action1);
    sample_action!(state.action_2, InputAction::Action2);
    sample_action!(state.action_3, InputAction::Action3);
    sample_action!(state.action_special, InputAction::Special);

    // --- Special toggles ---
    sample_action!(state.mode_debug, InputAction::ToggleDebug);
    sample_action!(state.fullscreen_toggle, InputAction::ToggleFullscreen);

    // --- Mouse wheel (analog scroll) ---
    state.scroll_y = rl.get_mouse_wheel_move();

    // --- Mouse position ---
    // Game-space: letterbox-corrected render-target coordinates
    // (0..render_width/height). Camera-independent — matches ScreenPosition
    // entity coordinates.
    let window_mouse_pos = rl.get_mouse_position();
    let game_mouse_pos = window_size.window_to_game_pos(
        window_mouse_pos,
        screen_size.w as u32,
        screen_size.h as u32,
    );
    state.mouse_x = game_mouse_pos.x;
    state.mouse_y = game_mouse_pos.y;

    // --- Raw left mouse button (not routed through InputBindings) ---
    // GUI hit-testing reacts to the literal left mouse button, independent
    // of any action rebinding.
    state.mouse_left_button = BoolState {
        active: rl.is_mouse_button_down(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT),
        just_pressed: rl.is_mouse_button_pressed(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT),
        just_released: rl.is_mouse_button_released(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT),
    };

    RawInputSnapshot { state }
}

// ---------------------------------------------------------------------------
// Logic-side apply (system)
// ---------------------------------------------------------------------------

/// Copy the latest [`RawInputSnapshot`] into [`InputState`], compute the
/// world-space mouse from the logic-owned camera, and emit input events.
///
/// Event semantics match the pre-split `update_input_state` exactly: one
/// [`InputEvent`] per action `just_pressed`/`just_released` edge;
/// `mode_debug`/`fullscreen_toggle` emit no `InputEvent` and instead trigger
/// [`SwitchDebugEvent`]/[`SwitchFullScreenEvent`] on `just_pressed`; the raw
/// `mouse_left_button` emits nothing.
pub fn apply_input_snapshot(
    latest: Res<LatestInputSnapshot>,
    mut input: ResMut<InputState>,
    camera: Res<Camera2DRes>,
    mut commands: Commands,
) {
    *input = latest.0.state.clone();

    // World-space: game-space projected through the current camera.
    // Matches MapPosition entity coordinates.
    let world_mouse_pos = screen_to_world2d(
        Vector2 {
            x: input.mouse_x,
            y: input.mouse_y,
        },
        &camera.0,
    );
    input.mouse_world_x = world_mouse_pos.x;
    input.mouse_world_y = world_mouse_pos.y;

    // Inline macro: emit InputEvents for one action's edges.
    macro_rules! emit_action {
        ($state:expr, $action:expr) => {{
            if $state.just_pressed {
                commands.trigger(InputEvent {
                    action: $action,
                    pressed: true,
                });
            }
            if $state.just_released {
                commands.trigger(InputEvent {
                    action: $action,
                    pressed: false,
                });
            }
        }};
    }

    emit_action!(input.maindirection_up, InputAction::MainDirectionUp);
    emit_action!(input.maindirection_down, InputAction::MainDirectionDown);
    emit_action!(input.maindirection_left, InputAction::MainDirectionLeft);
    emit_action!(input.maindirection_right, InputAction::MainDirectionRight);
    emit_action!(
        input.secondarydirection_up,
        InputAction::SecondaryDirectionUp
    );
    emit_action!(
        input.secondarydirection_down,
        InputAction::SecondaryDirectionDown
    );
    emit_action!(
        input.secondarydirection_left,
        InputAction::SecondaryDirectionLeft
    );
    emit_action!(
        input.secondarydirection_right,
        InputAction::SecondaryDirectionRight
    );
    emit_action!(input.action_back, InputAction::Back);
    emit_action!(input.action_1, InputAction::Action1);
    emit_action!(input.action_2, InputAction::Action2);
    emit_action!(input.action_3, InputAction::Action3);
    emit_action!(input.action_special, InputAction::Special);

    // mode_debug and fullscreen_toggle don't emit InputEvent; they trigger
    // their own dedicated events so existing observers don't need to change.
    if input.mode_debug.just_pressed {
        debug!("Debug mode key pressed");
        commands.trigger(SwitchDebugEvent {});
    }
    if input.fullscreen_toggle.just_pressed {
        debug!("Fullscreen toggle key pressed");
        commands.trigger(SwitchFullScreenEvent {});
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;
    use raylib::prelude::Camera2D;

    fn test_camera(target: (f32, f32), offset: (f32, f32), zoom: f32, rotation: f32) -> Camera2D {
        Camera2D {
            target: Vector2 {
                x: target.0,
                y: target.1,
            },
            offset: Vector2 {
                x: offset.0,
                y: offset.1,
            },
            rotation,
            zoom,
        }
    }

    #[test]
    fn screen_to_world2d_identity_camera() {
        let cam = test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0);
        let world = screen_to_world2d(Vector2 { x: 42.0, y: -7.0 }, &cam);
        assert!((world.x - 42.0).abs() < 1e-4);
        assert!((world.y - -7.0).abs() < 1e-4);
    }

    #[test]
    fn screen_to_world2d_target_offset_zoom() {
        // screen = (world - target) * zoom + offset  (rotation 0)
        // => world = (screen - offset) / zoom + target
        let cam = test_camera((100.0, 50.0), (400.0, 300.0), 2.0, 0.0);
        let center = screen_to_world2d(Vector2 { x: 400.0, y: 300.0 }, &cam);
        assert!((center.x - 100.0).abs() < 1e-3);
        assert!((center.y - 50.0).abs() < 1e-3);
        let right = screen_to_world2d(Vector2 { x: 500.0, y: 300.0 }, &cam);
        assert!((right.x - 150.0).abs() < 1e-3);
        assert!((right.y - 50.0).abs() < 1e-3);
    }

    #[test]
    fn screen_to_world2d_rotation() {
        // rotation 90°, zoom 1: world = R(-90°) * (screen - offset) + target
        // screen (10, 0) relative to offset maps to world (0, -10) + target.
        let cam = test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 90.0);
        let world = screen_to_world2d(Vector2 { x: 10.0, y: 0.0 }, &cam);
        assert!(world.x.abs() < 1e-3, "x = {}", world.x);
        assert!((world.y - -10.0).abs() < 1e-3, "y = {}", world.y);
    }

    #[derive(Resource, Default)]
    struct EventLog {
        input_events: Vec<(InputAction, bool)>,
        debug_switches: usize,
        fullscreen_switches: usize,
    }

    fn build_world(snapshot: RawInputSnapshot, camera: Camera2D) -> World {
        let mut world = World::new();
        world.insert_resource(InputState::default());
        world.insert_resource(LatestInputSnapshot(snapshot));
        world.insert_resource(Camera2DRes(camera));
        world.insert_resource(EventLog::default());
        world.add_observer(|trigger: On<InputEvent>, mut log: ResMut<EventLog>| {
            log.input_events.push((trigger.action, trigger.pressed));
        });
        world.add_observer(|_: On<SwitchDebugEvent>, mut log: ResMut<EventLog>| {
            log.debug_switches += 1;
        });
        world.add_observer(|_: On<SwitchFullScreenEvent>, mut log: ResMut<EventLog>| {
            log.fullscreen_switches += 1;
        });
        world
    }

    #[test]
    fn apply_copies_state_and_computes_mouse_world() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mouse_x = 500.0;
        snapshot.state.mouse_y = 300.0;
        snapshot.state.scroll_y = 1.5;
        snapshot.state.action_1.active = true;
        // camera: target (100, 50), offset (400, 300), zoom 2
        let mut world = build_world(snapshot, test_camera((100.0, 50.0), (400.0, 300.0), 2.0, 0.0));

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert_eq!(input.mouse_x, 500.0);
        assert_eq!(input.scroll_y, 1.5);
        assert!(input.action_1.active);
        assert!((input.mouse_world_x - 150.0).abs() < 1e-3);
        assert!((input.mouse_world_y - 50.0).abs() < 1e-3);
    }

    #[test]
    fn apply_emits_input_events_from_edges() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.action_1.just_pressed = true;
        snapshot.state.action_back.just_released = true;
        // active-without-edge must NOT emit
        snapshot.state.maindirection_up.active = true;
        // raw mouse button edges must NOT emit InputEvent
        snapshot.state.mouse_left_button.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));

        world.run_system_once(apply_input_snapshot).unwrap();

        let log = world.resource::<EventLog>();
        // Emission follows field order: action_back before action_1.
        assert_eq!(
            log.input_events,
            vec![(InputAction::Back, false), (InputAction::Action1, true)]
        );
        assert_eq!(log.debug_switches, 0);
        assert_eq!(log.fullscreen_switches, 0);
    }

    #[test]
    fn apply_triggers_debug_and_fullscreen_switches() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mode_debug.just_pressed = true;
        snapshot.state.fullscreen_toggle.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));

        world.run_system_once(apply_input_snapshot).unwrap();

        let log = world.resource::<EventLog>();
        assert_eq!(log.debug_switches, 1);
        assert_eq!(log.fullscreen_switches, 1);
        // toggles never emit plain InputEvents
        assert!(log.input_events.is_empty());
    }
}
