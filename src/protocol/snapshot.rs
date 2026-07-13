//! Triple-buffer transport for the sim thread's [`DrawableSnapshot`] (Phase
//! 7c).
//!
//! Replaces `RenderMsg::Snapshot(Box<DrawableSnapshot>)` on the crossbeam
//! channel: the sim thread writes the latest snapshot into
//! [`SnapshotPublisher`] (the `triple_buffer::Input` end), the render thread
//! reads [`SnapshotConsumer`] (the `Output` end) once per frame, latest-wins,
//! with no queue growth and no per-frame channel allocation. `Input<T>`/
//! `Output<T>` are `Send + Sync` when `T: Send` (verified against
//! `triple_buffer` 9.0.0's `SharedState`, which has `unsafe impl<T: Send>
//! Sync` and routes all mutation through `&mut self`), so both wrappers are
//! ordinary bevy `Resource`s — no `NonSend` needed, unlike the POC reference
//! which inserts both ends as non-send out of caution.
//!
//! The remaining `RenderMsg` variants (`Asset`, `Bindings`, `Quit`) stay on
//! the channel; see `docs/plans/phase7c-triple-buffer-snapshot.md` for the
//! resulting ordering hazard between the two transports (a snapshot can now
//! arrive referencing an asset whose `RenderMsg::Asset` is still in flight)
//! and its mitigations.

use bevy_ecs::prelude::*;
use triple_buffer::{Input, Output};

use crate::resources::drawable_snapshot::DrawableSnapshot;

/// Sim (logic) thread's write end of the snapshot triple buffer.
#[derive(Resource)]
pub struct SnapshotPublisher(pub Input<DrawableSnapshot>);

/// Render thread's read end of the snapshot triple buffer.
#[derive(Resource)]
pub struct SnapshotConsumer(pub Output<DrawableSnapshot>);
