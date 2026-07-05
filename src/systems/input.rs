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
//!   [`SwitchDebugEvent`]) from the snapshot's edge flags (F10's
//!   `SwitchFullScreenEvent` is render-side-only since Phase 5e). Logic-side
//!   work: no raylib handle involved.

use bevy_ecs::prelude::*;

use log::debug;

use raylib::math::Vector2;
use raylib::prelude::Camera2D;

use crate::events::input::{InputAction, InputEvent};
use crate::events::switchdebug::SwitchDebugEvent;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::input::{BoolState, InputState};
use crate::resources::input_bindings::{InputBinding, InputBindings};
use crate::resources::rawinput::{ImguiCaptureMirror, LatestInputSnapshot, RawInputSnapshot};
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
/// Event semantics match the pre-split `update_input_state` except for F10:
/// one [`InputEvent`] per action `just_pressed`/`just_released` edge;
/// `mode_debug` emits no `InputEvent` and instead triggers
/// [`SwitchDebugEvent`] on `just_pressed`; `fullscreen_toggle` triggers
/// nothing here (Phase 5e — the render loop handles F10 on its own world,
/// where `switch_fullscreen_observer` lives); the raw `mouse_left_button`
/// emits nothing.
///
/// Masks input meant for the F11 debug imgui overlay before any of the above
/// (Phase 6e, `ImguiCaptureMirror`) — mouse fields when `capture.mouse`,
/// keyboard-sourced digital fields when `capture.keyboard` — so gameplay
/// doesn't also react to clicks/keys meant for the debug panel:
/// - Mouse position/world-position are FROZEN at their pre-capture values
///   (not zeroed) while masked: zeroing would still hand `mouse_world_x/y` a
///   real, camera-projected coordinate (wherever screen-origin maps to), not
///   "no data," and any `MouseControlled` entity reads that field
///   unconditionally (`mouse_controller`, `src/systems/mousecontroller.rs`)
///   — freezing means such an entity simply stops following instead of
///   snapping to a bogus position.
/// - `mode_debug` (F11) is restored to its real, unmasked edge state after
///   the general keyboard mask: F11 is the debug overlay's own escape hatch
///   and must always work, even while an imgui widget in that same overlay
///   holds keyboard focus — masking it away would lock the user out of
///   closing the very panel that's capturing keyboard.
/// - Never suppresses `just_released`: a button masked mid-press must still
///   deliver its release, or gameplay sees a "stuck held" input once imgui
///   grabs focus mid-press.
///
/// **Known, accepted edge cases** (both are consequences of the one-frame
/// cross-thread lag on `capture` itself, documented at its send site in
/// `render_main_loop`, `engine_app.rs` — deliberately not eliminated, since
/// doing so would need the "elaborate same-frame reordering machinery" the
/// design explicitly avoids for a debug-only overlay):
/// - A click/keypress can leak through unmasked on the exact render frame the
///   cursor/focus first lands on an imgui widget (capture is still `false`
///   from the prior frame), and conversely a legitimate gameplay press can be
///   masked on the frame focus leaves (capture is still `true`).
/// - If a press and its release both land in the same coalesced input sample
///   while masked (e.g. a fast click during a backlog), the press is masked
///   but `just_released` is not (per the rule above), so an observer sees an
///   isolated release with no preceding press. No current `InputEvent`
///   consumer relies on press/release pairing (`menu_controller_observer`
///   ignores all release events), but a future one might.
pub fn apply_input_snapshot(
    latest: Res<LatestInputSnapshot>,
    imgui_capture: Res<ImguiCaptureMirror>,
    mut input: ResMut<InputState>,
    camera: Res<Camera2DRes>,
    mut commands: Commands,
) {
    let capture = imgui_capture.0;
    // Saved BEFORE the overwrite below so mouse masking can freeze at the
    // pre-capture position rather than leaking a bogus new one (see doc
    // comment above).
    let frozen_mouse = capture
        .mouse
        .then(|| (input.mouse_x, input.mouse_y, input.mouse_world_x, input.mouse_world_y));

    *input = latest.0.state.clone();

    if let Some((mouse_x, mouse_y, mouse_world_x, mouse_world_y)) = frozen_mouse {
        input.mouse_x = mouse_x;
        input.mouse_y = mouse_y;
        input.mouse_world_x = mouse_world_x;
        input.mouse_world_y = mouse_world_y;
        input.scroll_y = 0.0;
        input.mouse_left_button.force_inactive();
    } else {
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
    }

    if capture.keyboard {
        let mode_debug_edge = input.mode_debug;
        for bs in input.keyboard_bool_fields_mut() {
            bs.force_inactive();
        }
        // F11 must always work as the debug overlay's escape hatch -- see
        // doc comment above.
        input.mode_debug = mode_debug_edge;
    }

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

    // mode_debug doesn't emit InputEvent; it triggers its own dedicated event
    // so existing observers don't need to change. fullscreen_toggle triggers
    // NOTHING here since Phase 5e: F10 is render-side-only (the render loop
    // triggers SwitchFullScreenEvent on its own world from the sampled edge —
    // switch_fullscreen_observer and the FullScreen resource live there).
    if input.mode_debug.just_pressed {
        debug!("Debug mode key pressed");
        commands.trigger(SwitchDebugEvent {});
    }
}

/// Merge a newer raw input sample into `base` (Phase 5e input-backlog
/// coalescing).
///
/// When the logic thread falls behind the render thread, multiple
/// `LogicMsg::Input` messages can be pending for one logic pass. Processing
/// only the newest would silently drop `just_pressed`/`just_released` edges
/// carried by the intermediate samples, so the backlog is folded into one
/// snapshot: edges are OR-ed (an edge seen in ANY unprocessed sample fires
/// once), `active` and analog values take the newest sample, and `scroll_y`
/// (a per-frame delta) is summed.
///
/// IMPORTANT: only ever merge samples that no logic pass has consumed yet —
/// merging an already-applied snapshot would double-fire its edges.
pub fn merge_input_snapshots(base: &mut RawInputSnapshot, next: &RawInputSnapshot) {
    fn merge_bool(base: &mut BoolState, next: &BoolState) {
        base.just_pressed |= next.just_pressed;
        base.just_released |= next.just_released;
        base.active = next.active;
    }

    let b = &mut base.state;
    let n = &next.state;
    merge_bool(&mut b.maindirection_up, &n.maindirection_up);
    merge_bool(&mut b.maindirection_down, &n.maindirection_down);
    merge_bool(&mut b.maindirection_left, &n.maindirection_left);
    merge_bool(&mut b.maindirection_right, &n.maindirection_right);
    merge_bool(&mut b.secondarydirection_up, &n.secondarydirection_up);
    merge_bool(&mut b.secondarydirection_down, &n.secondarydirection_down);
    merge_bool(&mut b.secondarydirection_left, &n.secondarydirection_left);
    merge_bool(&mut b.secondarydirection_right, &n.secondarydirection_right);
    merge_bool(&mut b.action_back, &n.action_back);
    merge_bool(&mut b.action_1, &n.action_1);
    merge_bool(&mut b.action_2, &n.action_2);
    merge_bool(&mut b.action_3, &n.action_3);
    merge_bool(&mut b.mode_debug, &n.mode_debug);
    merge_bool(&mut b.fullscreen_toggle, &n.fullscreen_toggle);
    merge_bool(&mut b.action_special, &n.action_special);
    merge_bool(&mut b.mouse_left_button, &n.mouse_left_button);
    b.scroll_y += n.scroll_y;
    b.mouse_x = n.mouse_x;
    b.mouse_y = n.mouse_y;
    // mouse_world_x/y are 0.0 inside raw snapshots (camera-dependent, filled
    // in by apply_input_snapshot) — nothing to merge.
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::imgui_bridge::ImguiCaptureState;
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
        world.insert_resource(ImguiCaptureMirror::default());
        world.insert_resource(Camera2DRes(camera));
        world.insert_resource(EventLog::default());
        world.add_observer(|trigger: On<InputEvent>, mut log: ResMut<EventLog>| {
            log.input_events.push((trigger.action, trigger.pressed));
        });
        world.add_observer(|_: On<SwitchDebugEvent>, mut log: ResMut<EventLog>| {
            log.debug_switches += 1;
        });
        world.add_observer(|_: On<crate::events::switchfullscreen::SwitchFullScreenEvent>, mut log: ResMut<EventLog>| {
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
    fn apply_masks_mouse_left_press_when_imgui_captures_mouse_but_always_delivers_release() {
        // Frame 1: imgui has mouse capture (e.g. hovering the F11 debug
        // panel) while the left mouse button is freshly pressed -- gameplay
        // must not see it.
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mouse_left_button.active = true;
        snapshot.state.mouse_left_button.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: true,
            keyboard: false,
        }));

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert!(
            !input.mouse_left_button.active && !input.mouse_left_button.just_pressed,
            "mouse_left_button press must be masked while imgui has mouse capture"
        );

        // Frame 2: imgui releases capture (panel closed/unfocused) while the
        // same physical press is now releasing. This must NOT be masked --
        // gameplay needs the release to avoid seeing a "stuck held" button.
        let mut released = RawInputSnapshot::default();
        released.state.mouse_left_button.just_released = true;
        world.resource_mut::<LatestInputSnapshot>().0 = released;
        world.resource_mut::<ImguiCaptureMirror>().0 = ImguiCaptureState::default();

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert!(
            input.mouse_left_button.just_released,
            "just_released must never be masked, even though it followed a masked press"
        );
    }

    #[test]
    fn apply_masks_keyboard_actions_when_imgui_captures_keyboard() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.action_1.active = true;
        snapshot.state.action_1.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: false,
            keyboard: true,
        }));

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert!(
            !input.action_1.active && !input.action_1.just_pressed,
            "keyboard-sourced action must be masked while imgui has keyboard capture"
        );
    }


    #[test]
    fn apply_freezes_mouse_position_instead_of_leaking_screen_origin_world_pos_when_captured() {
        // Frame 1: no capture -- establish a real, non-origin cursor position.
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mouse_x = 500.0;
        snapshot.state.mouse_y = 300.0;
        // camera: target (100, 50), offset (400, 300), zoom 2 (same as
        // apply_copies_state_and_computes_mouse_world).
        let mut world = build_world(snapshot, test_camera((100.0, 50.0), (400.0, 300.0), 2.0, 0.0));
        world.run_system_once(apply_input_snapshot).unwrap();
        let established = world.resource::<InputState>().clone();
        assert_ne!(established.mouse_x, 0.0, "sanity: a real cursor position was established");

        // Frame 2: imgui captures the mouse, and the raw snapshot (as if the
        // cursor had moved) reports a different, non-zero position -- this
        // must NOT reach InputState. Freezing at the pre-capture position
        // (rather than zeroing, which would still project to a real
        // camera-space coordinate at screen-origin) is what stops
        // MouseControlled entities from snapping to a bogus position.
        let mut moved = RawInputSnapshot::default();
        moved.state.mouse_x = 999.0;
        moved.state.mouse_y = 999.0;
        world.resource_mut::<LatestInputSnapshot>().0 = moved;
        world.resource_mut::<ImguiCaptureMirror>().0 = ImguiCaptureState {
            mouse: true,
            keyboard: false,
        };

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert_eq!(
            (input.mouse_x, input.mouse_y),
            (established.mouse_x, established.mouse_y),
            "mouse position must freeze at its pre-capture value, not adopt the new raw sample"
        );
        assert_eq!(
            (input.mouse_world_x, input.mouse_world_y),
            (established.mouse_world_x, established.mouse_world_y),
            "mouse_world_x/y must also freeze -- MouseControlled entities read this field \
             unconditionally, so leaking a fresh (even zeroed) projection would still visibly \
             snap them to wherever screen-origin maps to in world space"
        );
    }

    #[test]
    fn apply_never_masks_mode_debug_so_f11_always_toggles_the_overlay() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mode_debug.active = true;
        snapshot.state.mode_debug.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.insert_resource(ImguiCaptureMirror(ImguiCaptureState {
            mouse: false,
            keyboard: true,
        }));

        world.run_system_once(apply_input_snapshot).unwrap();

        let input = world.resource::<InputState>();
        assert!(
            input.mode_debug.active && input.mode_debug.just_pressed,
            "F11 (mode_debug) must never be masked -- it's the debug overlay's own escape hatch \
             and must work even while an imgui widget in that overlay holds keyboard capture"
        );
        let log = world.resource::<EventLog>();
        assert_eq!(
            log.debug_switches, 1,
            "SwitchDebugEvent must still fire for F11 while keyboard capture is active"
        );
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

    // --- Phase 5e: input-backlog coalescing ---

    #[test]
    fn merge_preserves_edges_from_both_samples() {
        // Sample A: press edge. Sample B (newer): release edge, key up.
        let mut a = RawInputSnapshot::default();
        a.state.action_1.just_pressed = true;
        a.state.action_1.active = true;
        let mut b = RawInputSnapshot::default();
        b.state.action_1.just_released = true;
        b.state.action_1.active = false;

        merge_input_snapshots(&mut a, &b);

        // Both edges survive (each unprocessed sample's edge fires exactly
        // once in the single coalesced pass); `active` takes the newest.
        assert!(a.state.action_1.just_pressed);
        assert!(a.state.action_1.just_released);
        assert!(!a.state.action_1.active);
    }

    #[test]
    fn merge_takes_newest_analog_and_sums_scroll() {
        let mut a = RawInputSnapshot::default();
        a.state.mouse_x = 10.0;
        a.state.mouse_y = 20.0;
        a.state.scroll_y = 1.0;
        let mut b = RawInputSnapshot::default();
        b.state.mouse_x = 30.0;
        b.state.mouse_y = 40.0;
        b.state.scroll_y = -0.5;

        merge_input_snapshots(&mut a, &b);

        assert_eq!(a.state.mouse_x, 30.0);
        assert_eq!(a.state.mouse_y, 40.0);
        // scroll_y is a per-frame delta: the two frames' wheel movement adds up.
        assert_eq!(a.state.scroll_y, 0.5);
    }

    #[test]
    fn merged_backlog_fires_edge_exactly_once_through_apply() {
        // Regression guard for the double-fire risk: two backlogged samples
        // — one carrying the press edge, a newer one without it — coalesce
        // into ONE snapshot whose single apply emits the edge event once.
        let mut older = RawInputSnapshot::default();
        older.state.action_1.just_pressed = true;
        older.state.action_1.active = true;
        let mut newer = RawInputSnapshot::default();
        newer.state.action_1.active = true; // held, no new edge

        merge_input_snapshots(&mut older, &newer);

        let mut world = build_world(older, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));
        world.run_system_once(apply_input_snapshot).unwrap();

        let log = world.resource::<EventLog>();
        assert_eq!(
            log.input_events,
            vec![(InputAction::Action1, true)],
            "coalesced backlog must emit the press edge exactly once"
        );
    }

    #[test]
    fn apply_triggers_debug_switch_but_not_fullscreen() {
        let mut snapshot = RawInputSnapshot::default();
        snapshot.state.mode_debug.just_pressed = true;
        snapshot.state.fullscreen_toggle.just_pressed = true;
        let mut world = build_world(snapshot, test_camera((0.0, 0.0), (0.0, 0.0), 1.0, 0.0));

        world.run_system_once(apply_input_snapshot).unwrap();

        let log = world.resource::<EventLog>();
        assert_eq!(log.debug_switches, 1);
        // F10 is render-side-only since Phase 5e: the logic-side apply must
        // NOT trigger SwitchFullScreenEvent even when the edge is set.
        assert_eq!(log.fullscreen_switches, 0);
        // toggles never emit plain InputEvents
        assert!(log.input_events.is_empty());
    }
}
