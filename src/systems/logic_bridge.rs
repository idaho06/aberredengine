//! Logic-side channel systems for the render/logic thread split (Phase 5e).
//!
//! - [`forward_render_asset_cmds`] — drains the logic world's
//!   `Messages<RenderAssetCmd>` into `RenderMsg::Asset` (mirrors
//!   `forward_audio_cmds`, itself FIXED-scheduled since Phase 6d), pushed
//!   across the [`RenderTx`] channel. Moved onto the tail of the sim schedule
//!   in Phase 7c (`SimSet::Bookkeeping`, alongside `update_bevy_render_asset_cmds`)
//!   so it runs every sim tick regardless of the `present`/snapshot-publish
//!   decimation below — a tick's asset loads must not be starved by how often
//!   the snapshot itself publishes.
//! - [`send_drawable_snapshot`] — publishes this frame's [`DrawableSnapshot`]
//!   into the sim thread's [`SnapshotPublisher`] (the `triple_buffer` write
//!   end, Phase 7c; replaces the old `RenderMsg::Snapshot` channel send).
//!   Still `PRESENT`-scheduled, now decimated to `[simulation] snapshot_hz`
//!   rather than running once per received input sample.
//! - [`send_input_bindings_on_change`] — value-diffed mirror refresh for the
//!   render side's `sample_input_snapshot`, pushed across [`RenderTx`]. Moved
//!   to FIXED in Phase 6d (no cost or benefit either way -- moved purely for
//!   schedule uniformity), so it now runs once per FIXED substep rather than
//!   once per `PRESENT` pass; still catches both Lua rebinds and
//!   `GameCtx.input_bindings` mutations from FIXED-schedule Rust callbacks.

use bevy_ecs::prelude::*;

use crate::events::render_assets::RenderAssetCmd;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::render_logic::RenderMsg;
use crate::protocol::snapshot::SnapshotPublisher;
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::input_bindings::InputBindings;

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

/// Publish this frame's fully-settled [`DrawableSnapshot`] into the
/// [`SnapshotPublisher`] triple buffer (Phase 7c). The render loop reads the
/// latest publish once per frame (`Output::update`), so a slow render frame
/// simply redraws the last one and a fast render frame sees no new data.
pub fn send_drawable_snapshot(
    snapshot: Res<DrawableSnapshot>,
    mut publisher: ResMut<SnapshotPublisher>,
) {
    publisher.0.write(snapshot.clone());
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
