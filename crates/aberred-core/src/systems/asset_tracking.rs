//! Settles [`PendingAssets`] entries as load replies arrive and triggers the
//! matching [`AssetLoaded`]/[`AssetLoadFailed`] event.
//!
//! Loads become pending in the forwarders (`forward_render_asset_cmds`,
//! `forward_audio_cmds`). Render replies arrive as `LogicMsg`s, which the
//! logic thread's message drain settles with [`settle_load`]. Audio replies
//! arrive as `AudioMessage`s, settled by [`settle_audio_loads`].

use bevy_ecs::prelude::*;

use crate::events::asset::{AssetLoadFailed, AssetLoaded};
use crate::protocol::asset_kind::LoadOutcome;
use crate::protocol::audio::AudioMessage;
use crate::resources::pending_assets::PendingAssets;

/// Removes one pending load of `outcome.key` and triggers [`AssetLoaded`] or
/// [`AssetLoadFailed`]. The one place a load is settled.
pub fn settle_load(world: &mut World, outcome: LoadOutcome) {
    let LoadOutcome { kind, key, error } = outcome;
    world.resource_mut::<PendingAssets>().settle(kind, &key);
    match error {
        None => world.trigger(AssetLoaded { kind, key }),
        Some(error) => world.trigger(AssetLoadFailed { kind, key, error }),
    }
}

/// Settles the audio loads answered this tick. Runs after
/// `update_bevy_audio_messages`, so each reply is read once.
pub fn settle_audio_loads(mut reader: MessageReader<AudioMessage>, mut commands: Commands) {
    for outcome in reader.read().filter_map(AudioMessage::load_reply) {
        commands.queue(move |world: &mut World| settle_load(world, outcome));
    }
}
