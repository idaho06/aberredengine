//! Raw per-frame input snapshot (Phase 5a of the render/logic thread split).
//!
//! [`RawInputSnapshot`] is one render-frame's fully sampled input, produced by
//! [`sample_input_snapshot`](crate::systems::input::sample_input_snapshot) on
//! the thread that owns the raylib window and consumed by
//! [`apply_input_snapshot`](crate::systems::input::apply_input_snapshot) on
//! the logic side. It is fully `Send + Sync + Clone` — Phase 5e ships it
//! across the `LogicBridge` channel as `LogicMsg::Input(RawInputSnapshot)`.
//!
//! While the engine is still single-threaded (phases 5a–5d), the main loop
//! writes [`LatestInputSnapshot`] directly right before running
//! `apply_input_snapshot`.

use bevy_ecs::prelude::*;

use crate::resources::input::InputState;

/// One render-frame's raw input sample.
///
/// `state` is a fully resolved [`InputState`] — binding lookup and
/// `just_pressed`/`just_released` edge resolution happen at sample time, so
/// the consumer never needs `InputBindings` or raylib to interpret it —
/// EXCEPT `mouse_world_x`/`mouse_world_y`, which are left `0.0`: the
/// world-space projection requires the camera, which the logic side owns
/// (`apply_input_snapshot` fills them in from `Camera2DRes`).
///
/// Window dimensions are not carried here while both sides share one `World`
/// (`WindowSize` is refreshed by the main loop each frame); Phase 5e extends
/// this snapshot (or its channel message) with them when the threads split.
#[derive(Debug, Clone, Default)]
pub struct RawInputSnapshot {
    /// Resolved input state, minus the camera-dependent world-space mouse.
    pub state: InputState,
}

/// Resource holding the most recent [`RawInputSnapshot`].
///
/// Single-threaded (5a–5d): written by the main loop from an inline
/// `sample_input_snapshot` call. Post-cutover (5e): written from drained
/// `LogicMsg::Input` messages on the logic thread.
#[derive(Resource, Debug, Clone, Default)]
pub struct LatestInputSnapshot(pub RawInputSnapshot);
