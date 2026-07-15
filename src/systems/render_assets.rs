//! Shared `RenderAssetCmd` message-queue aging, called from both the sim
//! schedule's `SimSet::Bookkeeping` (logic world) and the render schedule
//! (render world) -- unlike the actual GL loader
//! ([`process_render_asset_cmds`](crate::systems::render::process_render_asset_cmds),
//! render-thread-only), this function ages a `Messages<RenderAssetCmd>`
//! queue that exists as a separate instance in each world, so it isn't
//! itself thread-exclusive code.

use bevy_ecs::prelude::*;

use crate::events::render_assets::RenderAssetCmd;

/// Advances the `RenderAssetCmd` message queue once per frame, so writes
/// from earlier this frame become readable. Mirrors `update_bevy_audio_cmds`.
pub fn update_bevy_render_asset_cmds(mut msgs: ResMut<Messages<RenderAssetCmd>>) {
    msgs.update();
}
