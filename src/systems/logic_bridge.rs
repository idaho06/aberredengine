//! Logic-side channel systems for the render/logic thread split (Phase 5e).
//!
//! These push state across the [`RenderTx`] channel:
//!
//! - [`forward_render_asset_cmds`] — drains the logic world's
//!   `Messages<RenderAssetCmd>` into `RenderMsg::Asset` (mirrors
//!   `forward_audio_cmds`, itself FIXED-scheduled since Phase 6d); ordered
//!   before [`send_drawable_snapshot`] so a frame's asset loads always reach
//!   the render thread before the snapshot that references them (single
//!   sender, FIFO channel). Runs at the tail of the logic thread's `PRESENT`
//!   schedule (called `VARIABLE` through Phase 6c).
//! - [`send_drawable_snapshot`] — ships this frame's [`DrawableSnapshot`].
//!   Also `PRESENT`-scheduled, immediately after `forward_render_asset_cmds`.
//! - [`send_input_bindings_on_change`] — value-diffed mirror refresh for the
//!   render side's `sample_input_snapshot`. Moved to FIXED in Phase 6d (no
//!   cost or benefit either way -- moved purely for schedule uniformity), so
//!   it now runs once per FIXED substep rather than once per `PRESENT` pass;
//!   still catches both Lua rebinds and `GameCtx.input_bindings` mutations
//!   from FIXED-schedule Rust callbacks.

use bevy_ecs::prelude::*;

use crate::protocol::render_logic::RenderMsg;
use crate::events::render_assets::RenderAssetCmd;
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::input_bindings::InputBindings;
use crate::protocol::endpoints::RenderTx;

/// Forward queued [`RenderAssetCmd`]s to the render thread. Send errors are
/// ignored (they only occur during shutdown, when the render side is gone).
pub fn forward_render_asset_cmds(
    tx: Res<RenderTx>,
    mut reader: MessageReader<RenderAssetCmd>,
) {
    for cmd in reader.read() {
        let _ = tx.0.send(RenderMsg::Asset(cmd.clone()));
    }
}

/// Ship this frame's fully-settled [`DrawableSnapshot`] to the render thread.
/// The render loop `try_iter()`s and keeps only the newest, so a slow render
/// frame simply skips intermediate snapshots.
pub fn send_drawable_snapshot(snapshot: Res<DrawableSnapshot>, tx: Res<RenderTx>) {
    let _ = tx.0.send(RenderMsg::Snapshot(Box::new(snapshot.clone())));
}

/// Send the render side a fresh [`InputBindings`] mirror whenever the
/// logic-side value changed (value diff against the last sent copy).
pub fn send_input_bindings_on_change(
    bindings: Res<InputBindings>,
    tx: Res<RenderTx>,
    mut last_sent: Local<Option<InputBindings>>,
) {
    if last_sent.as_ref() != Some(&*bindings) {
        *last_sent = Some(bindings.clone());
        let _ = tx.0.send(RenderMsg::Bindings(bindings.clone()));
    }
}
