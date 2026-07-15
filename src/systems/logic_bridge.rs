//! Logic-side channel systems for the render/logic thread split.
//!
//! - [`forward_render_asset_cmds`] — drains the logic world's
//!   `Messages<RenderAssetCmd>` into `RenderMsg::Asset`, pushed across the
//!   [`RenderTx`] channel. Runs on the tail of the sim schedule
//!   (`SimSet::Bookkeeping`, alongside `update_bevy_render_asset_cmds`) so it
//!   runs every sim tick regardless of the `present`/snapshot-publish
//!   decimation below — a tick's asset loads must not be starved by how often
//!   the snapshot itself publishes.
//! - [`send_drawable_snapshot`] — publishes this frame's [`DrawableSnapshot`]
//!   into the sim thread's [`SnapshotPublisher`] (the `triple_buffer` write
//!   end). Still `PRESENT`-scheduled, decimated to `[simulation] snapshot_hz`
//!   rather than running once per received input sample.
//!
//! `InputBindings` is logic-thread-only (no render-side mirror to refresh),
//! since binding resolution itself happens on the sim thread.

use bevy_ecs::prelude::*;

use crate::events::render_assets::RenderAssetCmd;
use crate::protocol::endpoints::RenderTx;
use crate::protocol::render_logic::RenderMsg;
use crate::protocol::snapshot::SnapshotPublisher;
use crate::resources::drawable_snapshot::DrawableSnapshot;

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
/// [`SnapshotPublisher`] triple buffer. The render loop reads the
/// latest publish once per frame (`Output::update`), so a slow render frame
/// simply redraws the last one and a fast render frame sees no new data.
pub fn send_drawable_snapshot(
    snapshot: Res<DrawableSnapshot>,
    mut publisher: ResMut<SnapshotPublisher>,
) {
    publisher.0.write(snapshot.clone());
}
