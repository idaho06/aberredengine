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

use crate::protocol::render_assets::RenderAssetCmd;
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
///
/// Writes into the triple buffer's existing input-side buffer via
/// `input_buffer_mut()`/`clone_into_buffer`/`publish()` instead of
/// `Input::write(snapshot.clone())` -- `write` move-assigns a freshly
/// cloned value over the input buffer, dropping whatever `Vec` capacity it
/// held from three publishes ago and reallocating all 8 drawable lists
/// every publish (~`snapshot_hz`, default ~60/s). `clone_into_buffer`
/// reuses that capacity instead; see its doc comment
/// (`resources/drawable_snapshot.rs`) for why a plain `clone_from` on the
/// whole struct wouldn't.
pub fn send_drawable_snapshot(
    snapshot: Res<DrawableSnapshot>,
    mut publisher: ResMut<SnapshotPublisher>,
) {
    let buf = publisher.0.input_buffer_mut();
    snapshot.clone_into_buffer(buf);
    publisher.0.publish();
}
