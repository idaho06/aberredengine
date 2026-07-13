//! Logic-side input resolution state (Phase 7d,
//! `docs/plans/phase7d-raw-input-ownership.md`).
//!
//! Binding resolution and `just_pressed`/`just_released` edge detection both
//! moved from the render thread to the logic thread: the render thread now
//! ships raw, unresolved [`RawDeviceSnapshot`](crate::protocol::raw_input::RawDeviceSnapshot)s
//! over a dedicated bounded channel, and
//! [`resolve_input_backlog`](crate::systems::input::resolve_input_backlog)
//! diffs each one against [`PrevRawSnapshot`] to compute edges.

use bevy_ecs::prelude::*;

use crate::protocol::raw_input::RawDeviceSnapshot;

/// The most recently resolved raw device state, persisted **across sim
/// ticks** (not reset per-tick) so edge detection has something to diff the
/// next backlogged sample against, even across a tick with no input at all.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct PrevRawSnapshot(pub RawDeviceSnapshot);

/// Logic-side mirror of the render thread's [`crate::resources::imgui_bridge::ImguiCaptureState`]
/// (Phase 6e). Updated from the newest queued `InputSample::capture` each
/// tick, and read by `resolve_input_backlog` to mask gameplay input while the
/// debug overlay has focus.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ImguiCaptureMirror(pub crate::resources::imgui_bridge::ImguiCaptureState);
