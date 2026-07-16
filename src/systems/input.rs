//! Input systems.
//!
//! The render/logic split for input is:
//!
//! - Raw device sampling (whole keyboard + mouse held state into a
//!   [`RawDeviceSnapshot`], with NO binding resolution and NO edges) is
//!   render-thread-only and lives in `crate::systems::render::input` — the
//!   only input code that touches the `RaylibHandle`.
//! - [`resolve_input_backlog`] — resolves a sim tick's queued
//!   [`RawDeviceSnapshot`]s against [`InputBindings`] and
//!   [`PrevRawSnapshot`], sequentially (oldest to newest, see its doc comment
//!   for why this can't be a merge-then-diff-once pass), into [`InputState`],
//!   then applies imgui capture masking and emits [`InputEvent`]/
//!   [`SwitchDebugEvent`] once against the tick's final resolved state.
//!   Logic-side work: no raylib handle involved. Called directly by the
//!   logic thread's loop, not as a scheduled system (there's no fixed set of
//!   `Res`/`ResMut` params that would fit a backlog of variable length).

use bevy_ecs::prelude::*;

use log::debug;

use raylib::math::Vector2;
use raylib::prelude::Camera2D;

use crate::events::input::{InputAction, InputEvent};
use crate::events::switchdebug::SwitchDebugEvent;
use crate::protocol::raw_input::RawDeviceSnapshot;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::gameconfig::GameConfig;
use crate::resources::input::{BoolState, InputState};
use crate::resources::input_bindings::{AxisDirection, InputBinding, InputBindings};
use crate::resources::rawinput::{ImguiCaptureMirror, PrevRawSnapshot};
use crate::resources::screensize::ScreenSize;
use crate::resources::windowsize::WindowSize;

/// Shared engine-wide axis-to-digital crossing threshold for
/// [`InputBinding::GamepadAxis`] (see that variant's doc comment for why
/// this isn't a per-binding field).
const GAMEPAD_AXIS_THRESHOLD: f32 = 0.5;

/// Zero out `v` if its magnitude is below `deadzone` — a simple
/// clamp-to-zero deadzone (not a rescale of the remaining range).
fn apply_deadzone(v: f32, deadzone: f32) -> f32 {
    if v.abs() < deadzone { 0.0 } else { v }
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
// Logic-side resolution (plain function, not a system)
// ---------------------------------------------------------------------------

/// Whether `action` is active in `raw`, per its current [`InputBindings`].
fn action_active(
    raw: &RawDeviceSnapshot,
    bindings: &InputBindings,
    action: InputAction,
    deadzone: f32,
) -> bool {
    bindings.get_bindings(action).iter().any(|b| match b {
        InputBinding::Keyboard(k) => raw.is_key_down(*k as u32),
        InputBinding::MouseButton(m) => raw.is_mouse_button_down(*m as u8),
        InputBinding::GamepadButton { pad, button } => raw
            .gamepads
            .get(*pad as usize)
            .is_some_and(|g| g.is_button_down(*button as u32)),
        InputBinding::GamepadAxis {
            pad,
            axis,
            direction,
        } => {
            let Some(gamepad) = raw.gamepads.get(*pad as usize) else {
                return false;
            };
            let v = apply_deadzone(gamepad.axes[*axis as usize], deadzone);
            match direction {
                AxisDirection::Positive => v > GAMEPAD_AXIS_THRESHOLD,
                AxisDirection::Negative => v < -GAMEPAD_AXIS_THRESHOLD,
            }
        }
    })
}

/// Diff one bound action between `prev` and `sample`, OR-ing the resulting
/// edge into `state` (never clearing an edge an earlier sample in this
/// tick's backlog already set) and taking `sample`'s active/held state
/// (last-sample-wins across the backlog).
fn resolve_action(
    state: &mut BoolState,
    prev: &RawDeviceSnapshot,
    sample: &RawDeviceSnapshot,
    bindings: &InputBindings,
    action: InputAction,
    deadzone: f32,
) {
    let was = action_active(prev, bindings, action, deadzone);
    let now = action_active(sample, bindings, action, deadzone);
    state.apply_edge(was, now);
}

/// Diff the raw left mouse button (not routed through [`InputBindings`] —
/// GUI hit-testing always reacts to the literal left button) the same way as
/// [`resolve_action`].
fn resolve_mouse_left(state: &mut BoolState, prev: &RawDeviceSnapshot, sample: &RawDeviceSnapshot) {
    let left = raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT as u8;
    let was = prev.is_mouse_button_down(left);
    let now = sample.is_mouse_button_down(left);
    state.apply_edge(was, now);
}

/// Resolve one raw sample's 14 bound actions + the raw left mouse button into
/// `input`, diffed against `prev`.
fn resolve_sample_into(
    input: &mut InputState,
    prev: &RawDeviceSnapshot,
    sample: &RawDeviceSnapshot,
    bindings: &InputBindings,
    deadzone: f32,
) {
    macro_rules! resolve {
        ($field:ident, $action:expr) => {
            resolve_action(&mut input.$field, prev, sample, bindings, $action, deadzone)
        };
    }
    resolve!(maindirection_up, InputAction::MainDirectionUp);
    resolve!(maindirection_down, InputAction::MainDirectionDown);
    resolve!(maindirection_left, InputAction::MainDirectionLeft);
    resolve!(maindirection_right, InputAction::MainDirectionRight);
    resolve!(secondarydirection_up, InputAction::SecondaryDirectionUp);
    resolve!(secondarydirection_down, InputAction::SecondaryDirectionDown);
    resolve!(secondarydirection_left, InputAction::SecondaryDirectionLeft);
    resolve!(secondarydirection_right, InputAction::SecondaryDirectionRight);
    resolve!(action_back, InputAction::Back);
    resolve!(action_1, InputAction::Action1);
    resolve!(action_2, InputAction::Action2);
    resolve!(action_3, InputAction::Action3);
    resolve!(action_special, InputAction::Special);
    resolve!(mode_debug, InputAction::ToggleDebug);
    resolve!(fullscreen_toggle, InputAction::ToggleFullscreen);
    resolve_mouse_left(&mut input.mouse_left_button, prev, sample);
}

/// Resolve a sim tick's backlog of raw device samples (oldest to newest)
/// into [`InputState`], then apply imgui capture masking and emit
/// [`InputEvent`]/[`SwitchDebugEvent`] once against the tick's final resolved
/// state.
///
/// **Why sequential per-sample resolution, not merge-then-diff-once:** edge
/// computation is a *diff* against the previous raw state, not a merge of
/// already-resolved edges. Diffing two *raw* snapshots directly
/// lose any transition in between: e.g. a key going down, up, then down again
/// across 3 backlogged samples must fire two separate `just_pressed` edges,
/// but a first-vs-last diff would see "still down" and report none. Each
/// sample is therefore resolved against the immediately preceding one
/// (starting from [`PrevRawSnapshot`], which persists **across ticks**, not
/// just within one), with edges OR-ed into the tick's `InputState` so an
/// edge fired by any backlogged sample is delivered exactly once this tick.
/// `active`/analog fields take the newest sample (last-sample-wins).
///
/// Multi-binding actions get a sane edge semantic: rolling from one bound key
/// to another (e.g. both Z and X bound to the same action, tap-hold-Z-then-X)
/// never double-fires `just_pressed`, since `active` (any-bound-down) stays
/// `true` across the handoff.
///
/// Also resolves `fullscreen_toggle` (F10) as an ordinary bound action — the
/// caller reads `InputState.fullscreen_toggle.just_pressed` after this
/// returns and sends `RenderMsg::ToggleFullscreen` itself, keeping this
/// function free of channel sends.
pub fn resolve_input_backlog(world: &mut World, samples: &[RawDeviceSnapshot]) {
    if samples.is_empty() {
        return;
    }

    let capture = world.resource::<ImguiCaptureMirror>().0;
    let camera = world.resource::<Camera2DRes>().0;
    // Set by the caller from this backlog's newest sample just before
    // calling (`logic_thread_main`), so it's already the right value to
    // letterbox-correct the mouse position against — no need to rebuild a
    // second `WindowSize` from `samples.last()` here.
    let window_size = *world.resource::<WindowSize>();
    let screen_size = *world.resource::<ScreenSize>();
    let deadzone = world.resource::<GameConfig>().gamepad_deadzone;
    let mut prev_raw = world.resource::<PrevRawSnapshot>().0;

    world.resource_scope(|world, mut input: Mut<InputState>| {
        // Borrowed (not cloned) for the resolve loop below; the borrow ends
        // by NLL before `world.trigger(...)` needs `world` mutably again.
        let bindings = world.resource::<InputBindings>();

        // Saved BEFORE this tick's mouse fields are overwritten below, so
        // mouse masking can freeze at the pre-capture position rather than
        // leaking a bogus new one (see the imgui-capture doc comment below).
        let frozen_mouse = capture
            .mouse
            .then(|| (input.mouse_x, input.mouse_y, input.mouse_world_x, input.mouse_world_y));

        input.scroll_y = 0.0;
        for sample in samples {
            resolve_sample_into(&mut input, &prev_raw, sample, bindings, deadzone);
            input.scroll_y += sample.scroll_y;
            prev_raw = *sample;
        }

        // Newest sample's mouse position, letterbox-corrected into
        // game-space (matches ScreenPosition entity coordinates).
        let newest = samples.last().expect("checked non-empty above");

        // Pad 0's raw analog state -- last-sample-wins (same tier as
        // mouse_x/mouse_y), NOT summed like scroll_y. Deadzone is
        // deliberately NOT applied here: these are the raw values surfaced
        // to gameplay/Lua, kept true for a future calibration UI; deadzone
        // only affects action_active's digital-threshold resolution above.
        input.gamepad_connected = newest.gamepads[0].connected;
        input.gamepad_axes = newest.gamepads[0].axes;
        let game_mouse_pos = window_size.window_to_game_pos(
            Vector2 {
                x: newest.mouse_x,
                y: newest.mouse_y,
            },
            screen_size.w as u32,
            screen_size.h as u32,
        );
        input.mouse_x = game_mouse_pos.x;
        input.mouse_y = game_mouse_pos.y;

        // --- Imgui debug-overlay input capture (Phase 6e; see
        // apply_input_snapshot's original doc comment, ported verbatim) ---
        // Masks input meant for the F11 debug imgui overlay before events
        // are emitted, so gameplay doesn't also react to clicks/keys meant
        // for the debug panel:
        // - Mouse position/world-position are FROZEN at their pre-capture
        //   values (not zeroed) while masked: zeroing would still hand
        //   `mouse_world_x/y` a real, camera-projected coordinate (wherever
        //   screen-origin maps to), and any `MouseControlled` entity reads
        //   that field unconditionally (`mouse_controller`) — freezing means
        //   such an entity simply stops following instead of snapping to a
        //   bogus position.
        // - `mode_debug` (F11) and `fullscreen_toggle` (F10) are restored to
        //   their real, unmasked edge state after the general keyboard mask:
        //   F11 is the debug overlay's own escape hatch and must always
        //   work, even while an imgui widget in that overlay holds keyboard
        //   focus; F10 mirrors its pre-Phase-7d behavior, where fullscreen
        //   was resolved entirely render-side and never interacted with
        //   imgui capture at all.
        // - Never suppresses `just_released`: a button masked mid-press must
        //   still deliver its release, or gameplay sees a "stuck held" input
        //   once imgui grabs focus mid-press.
        if let Some((mouse_x, mouse_y, mouse_world_x, mouse_world_y)) = frozen_mouse {
            input.mouse_x = mouse_x;
            input.mouse_y = mouse_y;
            input.mouse_world_x = mouse_world_x;
            input.mouse_world_y = mouse_world_y;
            input.scroll_y = 0.0;
            input.mouse_left_button.force_inactive();
        } else {
            let world_mouse_pos = screen_to_world2d(
                Vector2 {
                    x: input.mouse_x,
                    y: input.mouse_y,
                },
                &camera,
            );
            input.mouse_world_x = world_mouse_pos.x;
            input.mouse_world_y = world_mouse_pos.y;
        }

        if capture.keyboard {
            let mode_debug_edge = input.mode_debug;
            let fullscreen_edge = input.fullscreen_toggle;
            for bs in input.keyboard_bool_fields_mut() {
                bs.force_inactive();
            }
            input.mode_debug = mode_debug_edge;
            input.fullscreen_toggle = fullscreen_edge;
        }

        // Inline macro: emit InputEvents for one action's edges.
        macro_rules! emit_action {
            ($state:expr, $action:expr) => {{
                if $state.just_pressed {
                    world.trigger(InputEvent {
                        action: $action,
                        pressed: true,
                    });
                }
                if $state.just_released {
                    world.trigger(InputEvent {
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

        // mode_debug doesn't emit InputEvent; it triggers its own dedicated
        // event so existing observers don't need to change. fullscreen_toggle
        // likewise triggers no InputEvent — the caller reads the resolved
        // InputState directly and sends RenderMsg::ToggleFullscreen.
        if input.mode_debug.just_pressed {
            debug!("Debug mode key pressed");
            world.trigger(SwitchDebugEvent {});
        }
    });

    world.resource_mut::<PrevRawSnapshot>().0 = prev_raw;
}

#[cfg(test)]
mod tests {
    use super::*;
    use raylib::ffi::{GamepadAxis, GamepadButton};
    use crate::resources::render::imgui_bridge::ImguiCaptureState;
    use raylib::ffi::KeyboardKey;
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
    }

    fn build_world(camera: Camera2D) -> World {
        let mut world = World::new();
        world.insert_resource(InputState::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(GameConfig::default());
        world.insert_resource(PrevRawSnapshot::default());
        world.insert_resource(ImguiCaptureMirror::default());
        world.insert_resource(Camera2DRes(camera));
        world.insert_resource(ScreenSize { w: 640, h: 360 });
        world.insert_resource(WindowSize { w: 640, h: 360 });
        world.insert_resource(EventLog::default());
        world.add_observer(|trigger: On<InputEvent>, mut log: ResMut<EventLog>| {
            log.input_events.push((trigger.action, trigger.pressed));
        });
        world.add_observer(|_: On<SwitchDebugEvent>, mut log: ResMut<EventLog>| {
            log.debug_switches += 1;
        });
        world
    }

    /// An empty raw snapshot at the standard 640x360 test window size (every
    /// test in this module uses the same size; see `build_world`'s matching
    /// `WindowSize`/`ScreenSize`).
    fn raw() -> RawDeviceSnapshot {
        RawDeviceSnapshot {
            window_w: 640,
            window_h: 360,
            ..Default::default()
        }
    }

    /// A raw snapshot with only `action` (bound to `key` by default bindings)
    /// held down, at the given window-space mouse position.
    fn raw_with_key_down(key: KeyboardKey, mouse: (f32, f32)) -> RawDeviceSnapshot {
        let mut raw = RawDeviceSnapshot {
            mouse_x: mouse.0,
            mouse_y: mouse.1,
            ..raw()
        };
        raw.set_key(key as u32);
        raw
    }

    #[test]
    fn resolve_computes_mouse_world_and_scroll() {
        // camera: target (100, 50), offset (400, 300), zoom 2 (window ==
        // screen size, so letterbox is a no-op and game coords == window
        // coords).
        let mut world = build_world(test_camera((100.0, 50.0), (400.0, 300.0), 2.0, 0.0));
        let mut sample = raw_with_key_down(KeyboardKey::KEY_SPACE, (500.0, 300.0));
        sample.scroll_y = 1.5;

        resolve_input_backlog(&mut world, &[sample]);

        let input = world.resource::<InputState>();
        assert_eq!(input.mouse_x, 500.0);
        assert_eq!(input.scroll_y, 1.5);
        assert!(input.action_1.active);
        assert!(input.action_1.just_pressed);
        assert!((input.mouse_world_x - 150.0).abs() < 1e-3);
        assert!((input.mouse_world_y - 50.0).abs() < 1e-3);
    }

    #[test]
    fn resolve_fires_press_then_release_across_ticks() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));

        let down = raw_with_key_down(KeyboardKey::KEY_SPACE, (0.0, 0.0));
        resolve_input_backlog(&mut world, &[down]);
        assert!(world.resource::<InputState>().action_1.just_pressed);
        assert!(world.resource::<InputState>().action_1.active);

        // Same key still down: no new edge (active stays true).
        let held = raw_with_key_down(KeyboardKey::KEY_SPACE, (0.0, 0.0));
        // Simulate the per-tick edge clear that `run_sim_tick` performs.
        world.resource_mut::<InputState>().clear_edges();
        resolve_input_backlog(&mut world, &[held]);
        assert!(!world.resource::<InputState>().action_1.just_pressed);
        assert!(world.resource::<InputState>().action_1.active);

        // Key released.
        world.resource_mut::<InputState>().clear_edges();
        let up = raw();
        resolve_input_backlog(&mut world, &[up]);
        let input = world.resource::<InputState>();
        assert!(input.action_1.just_released);
        assert!(!input.action_1.active);
    }

    #[test]
    fn resolve_backlog_within_one_tick_fires_press_and_release_from_intermediate_sample() {
        // A tap that lands entirely within one tick's backlog: down in
        // sample 1, up again in sample 2 -- a first-vs-last diff would see
        // "still up" (matching PrevRawSnapshot's initial down=false) and
        // report NOTHING, but the sequential per-sample model must still
        // fire both edges since the transition happened in between.
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let down = raw_with_key_down(KeyboardKey::KEY_SPACE, (0.0, 0.0));
        let up = raw();

        resolve_input_backlog(&mut world, &[down, up]);

        let input = world.resource::<InputState>();
        assert!(input.action_1.just_pressed, "press edge must survive the intermediate transition");
        assert!(input.action_1.just_released, "release edge must survive the intermediate transition");
        assert!(!input.action_1.active);
    }

    #[test]
    fn resolve_rolling_between_two_bound_keys_does_not_double_fire_press() {
        // Action1's default bindings include Space + mouse-left; rebind to
        // two keyboard keys so a roll between them is testable without a
        // mouse button.
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        {
            let mut bindings = world.resource_mut::<InputBindings>();
            bindings.rebind(InputAction::Action1, InputBinding::Keyboard(KeyboardKey::KEY_Z));
            bindings.add_binding(InputAction::Action1, InputBinding::Keyboard(KeyboardKey::KEY_X));
        }

        let mut z_down = raw();
        z_down.set_key(KeyboardKey::KEY_Z as u32);
        resolve_input_backlog(&mut world, &[z_down]);
        assert!(world.resource::<InputState>().action_1.just_pressed);
        world.resource_mut::<InputState>().clear_edges();

        // Roll from Z to X within the same tick's backlog: Z+X both down,
        // then only X down. `active` (any-bound-down) never goes false, so
        // no new just_pressed should fire.
        let mut both_down = raw();
        both_down.set_key(KeyboardKey::KEY_Z as u32);
        both_down.set_key(KeyboardKey::KEY_X as u32);
        let mut x_only = raw();
        x_only.set_key(KeyboardKey::KEY_X as u32);

        resolve_input_backlog(&mut world, &[both_down, x_only]);

        let input = world.resource::<InputState>();
        assert!(
            !input.action_1.just_pressed,
            "rolling handoff between two bound keys must not double-fire just_pressed"
        );
        assert!(input.action_1.active, "action must still read active through the handoff");
    }

    #[test]
    fn resolve_masks_mouse_left_press_when_imgui_captures_mouse_but_always_delivers_release() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: true,
            keyboard: false,
        }));
        let mut pressed = raw();
        pressed.set_mouse_button(raylib::ffi::MouseButton::MOUSE_BUTTON_LEFT as u8);

        resolve_input_backlog(&mut world, &[pressed]);

        let input = world.resource::<InputState>();
        assert!(
            !input.mouse_left_button.active && !input.mouse_left_button.just_pressed,
            "mouse_left_button press must be masked while imgui has mouse capture"
        );

        // Release while capture is gone: must NOT be masked.
        world.resource_mut::<InputState>().clear_edges();
        world.resource_mut::<ImguiCaptureMirror>().0 = ImguiCaptureState::default();
        let released = raw();
        resolve_input_backlog(&mut world, &[released]);

        let input = world.resource::<InputState>();
        assert!(
            input.mouse_left_button.just_released,
            "just_released must never be masked, even though it followed a masked press"
        );
    }

    #[test]
    fn resolve_masks_keyboard_actions_when_imgui_captures_keyboard() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: false,
            keyboard: true,
        }));
        let sample = raw_with_key_down(KeyboardKey::KEY_SPACE, (0.0, 0.0));

        resolve_input_backlog(&mut world, &[sample]);

        let input = world.resource::<InputState>();
        assert!(
            !input.action_1.active && !input.action_1.just_pressed,
            "keyboard-sourced action must be masked while imgui has keyboard capture"
        );
    }

    #[test]
    fn resolve_freezes_mouse_position_instead_of_leaking_screen_origin_world_pos_when_captured() {
        let mut world = build_world(test_camera((100.0, 50.0), (400.0, 300.0), 2.0, 0.0));
        let established_sample = RawDeviceSnapshot {
            mouse_x: 500.0,
            mouse_y: 300.0,
            ..raw()
        };
        resolve_input_backlog(&mut world, &[established_sample]);
        let established = world.resource::<InputState>().clone();
        assert_ne!(established.mouse_x, 0.0, "sanity: a real cursor position was established");

        world.resource_mut::<InputState>().clear_edges();
        world.resource_mut::<ImguiCaptureMirror>().0 = ImguiCaptureState {
            mouse: true,
            keyboard: false,
        };
        let moved_sample = RawDeviceSnapshot {
            mouse_x: 999.0,
            mouse_y: 999.0,
            ..raw()
        };
        resolve_input_backlog(&mut world, &[moved_sample]);

        let input = world.resource::<InputState>();
        assert_eq!(
            (input.mouse_x, input.mouse_y),
            (established.mouse_x, established.mouse_y),
            "mouse position must freeze at its pre-capture value, not adopt the new raw sample"
        );
        assert_eq!(
            (input.mouse_world_x, input.mouse_world_y),
            (established.mouse_world_x, established.mouse_world_y),
            "mouse_world_x/y must also freeze"
        );
    }

    #[test]
    fn resolve_never_masks_mode_debug_or_fullscreen_so_f11_and_f10_always_work() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: false,
            keyboard: true,
        }));
        let mut sample = raw();
        sample.set_key(KeyboardKey::KEY_F11 as u32);
        sample.set_key(KeyboardKey::KEY_F10 as u32);

        resolve_input_backlog(&mut world, &[sample]);

        let input = world.resource::<InputState>();
        assert!(
            input.mode_debug.active && input.mode_debug.just_pressed,
            "F11 must never be masked -- it's the debug overlay's own escape hatch"
        );
        assert!(
            input.fullscreen_toggle.just_pressed,
            "F10 must never be masked -- it never interacted with imgui capture pre-7d either"
        );
        let log = world.resource::<EventLog>();
        assert_eq!(log.debug_switches, 1);
    }

    #[test]
    fn resolve_emits_input_events_from_edges_in_field_order() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let mut sample = raw();
        // action_1 pressed (Space), action_back released is impossible to
        // seed from a single raw snapshot (release needs a prior press) --
        // seed the prior press via a first sample, then release via a
        // second, and confirm ordering follows struct field declaration
        // order (action_back before action_1).
        sample.set_key(KeyboardKey::KEY_ESCAPE as u32); // action_back
        resolve_input_backlog(&mut world, &[sample]);
        world.resource_mut::<InputState>().clear_edges();
        world.resource_mut::<EventLog>().input_events.clear();

        let mut release_back_press_action1 = raw();
        release_back_press_action1.set_key(KeyboardKey::KEY_SPACE as u32);
        resolve_input_backlog(&mut world, &[release_back_press_action1]);

        let log = world.resource::<EventLog>();
        assert_eq!(
            log.input_events,
            vec![(InputAction::Back, false), (InputAction::Action1, true)]
        );
        assert_eq!(log.debug_switches, 0);
    }

    #[test]
    fn resolve_empty_backlog_is_a_no_op() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        resolve_input_backlog(&mut world, &[]);
        assert_eq!(world.resource::<EventLog>().input_events.len(), 0);
    }

    /// A raw snapshot with pad 0 connected, `button` (if given) held down and
    /// `axes` set (defaulting to all-zero).
    fn raw_with_gamepad(button: Option<u32>, axes: [f32; 6]) -> RawDeviceSnapshot {
        let mut r = raw();
        r.gamepads[0].connected = true;
        r.gamepads[0].axes = axes;
        if let Some(b) = button {
            r.gamepads[0].set_button(b);
        }
        r
    }

    #[test]
    fn resolve_gamepad_button_press_and_release_fires_edges() {
        // Default bindings map pad0's RIGHT_FACE_DOWN to Action1.
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let down = raw_with_gamepad(
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN as u32),
            [0.0; 6],
        );
        resolve_input_backlog(&mut world, &[down]);
        let input = world.resource::<InputState>();
        assert!(input.action_1.just_pressed);
        assert!(input.action_1.active);

        world.resource_mut::<InputState>().clear_edges();
        let up = raw_with_gamepad(None, [0.0; 6]);
        resolve_input_backlog(&mut world, &[up]);
        let input = world.resource::<InputState>();
        assert!(input.action_1.just_released);
        assert!(!input.action_1.active);
    }

    #[test]
    fn resolve_gamepad_axis_threshold_crossing_fires_both_directions() {
        // Default bindings map pad0's LEFT_X axis to MainDirectionRight
        // (positive) / MainDirectionLeft (negative).
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let neutral = raw_with_gamepad(None, [0.0; 6]);
        resolve_input_backlog(&mut world, &[neutral]);
        assert!(!world.resource::<InputState>().maindirection_right.active);

        world.resource_mut::<InputState>().clear_edges();
        let mut axes = [0.0; 6];
        axes[GamepadAxis::GAMEPAD_AXIS_LEFT_X as usize] = 0.8;
        let pushed_right = raw_with_gamepad(None, axes);
        resolve_input_backlog(&mut world, &[pushed_right]);
        let input = world.resource::<InputState>();
        assert!(input.maindirection_right.just_pressed);
        assert!(input.maindirection_right.active);

        world.resource_mut::<InputState>().clear_edges();
        let back_to_neutral = raw_with_gamepad(None, [0.0; 6]);
        resolve_input_backlog(&mut world, &[back_to_neutral]);
        let input = world.resource::<InputState>();
        assert!(input.maindirection_right.just_released);
        assert!(!input.maindirection_right.active);
    }

    #[test]
    fn resolve_gamepad_axis_within_deadzone_reads_as_inactive() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        // Default deadzone is 0.15; 0.1 is inside it.
        let mut axes = [0.0; 6];
        axes[GamepadAxis::GAMEPAD_AXIS_LEFT_X as usize] = 0.1;
        let sample = raw_with_gamepad(None, axes);
        resolve_input_backlog(&mut world, &[sample]);
        let input = world.resource::<InputState>();
        assert!(!input.maindirection_right.active);
        assert!(!input.maindirection_left.active);
    }

    #[test]
    fn resolve_disconnected_pad_reads_as_all_zero_not_stale() {
        // Connected + button held in sample 1, then a disconnected
        // (all-zero, connected=false) sample in the SAME tick's backlog --
        // must fire just_released, not freeze at active=true.
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let connected_and_pressed = raw_with_gamepad(
            Some(GamepadButton::GAMEPAD_BUTTON_RIGHT_FACE_DOWN as u32),
            [0.0; 6],
        );
        let disconnected = raw(); // gamepads[0] defaults to RawGamepad::default()

        resolve_input_backlog(&mut world, &[connected_and_pressed, disconnected]);
        let input = world.resource::<InputState>();
        assert!(input.action_1.just_pressed);
        assert!(input.action_1.just_released);
        assert!(!input.action_1.active);
        assert!(!input.gamepad_connected);
        assert_eq!(input.gamepad_axes, [0.0; 6]);
    }

    #[test]
    fn resolve_gamepad_axes_and_connected_are_last_sample_wins() {
        let mut world = build_world(test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        let mut axes1 = [0.0; 6];
        axes1[GamepadAxis::GAMEPAD_AXIS_LEFT_X as usize] = 0.9;
        let sample1 = raw_with_gamepad(None, axes1);
        let mut axes2 = [0.0; 6];
        axes2[GamepadAxis::GAMEPAD_AXIS_LEFT_X as usize] = 0.3;
        let sample2 = raw_with_gamepad(None, axes2);

        resolve_input_backlog(&mut world, &[sample1, sample2]);
        let input = world.resource::<InputState>();
        assert!(input.gamepad_connected);
        assert_eq!(input.gamepad_axes[GamepadAxis::GAMEPAD_AXIS_LEFT_X as usize], 0.3);
    }
}
