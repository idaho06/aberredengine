//! Settles [`PendingAssets`] entries as load replies arrive, keeps
//! [`LoadedAssets`] current, and triggers the matching
//! [`AssetLoaded`]/[`AssetLoadFailed`] event.
//!
//! Loads become pending in the forwarders (`forward_render_asset_cmds`,
//! `forward_audio_cmds`). Render replies arrive as `LogicMsg`s, which the
//! logic thread's message drain handles. Audio replies arrive as
//! `AudioMessage`s, handled by [`track_audio_asset_replies`]. Both apply a
//! reply's `asset_change()` to [`LoadedAssets`], then settle its
//! `load_reply()` with [`settle_load`].

use bevy_ecs::prelude::*;

use crate::events::asset::{AssetLoadFailed, AssetLoaded};
use crate::protocol::asset_kind::LoadOutcome;
use crate::protocol::audio::AudioMessage;
use crate::resources::loaded_assets::LoadedAssets;
use crate::resources::pending_assets::PendingAssets;

/// Removes one pending load of `outcome.key` and triggers [`AssetLoaded`]
/// or [`AssetLoadFailed`]. The one place a load is settled.
pub fn settle_load(world: &mut World, outcome: LoadOutcome) {
    let LoadOutcome { kind, key, error } = outcome;
    world.resource_mut::<PendingAssets>().settle(kind, &key);
    match error {
        None => world.trigger(AssetLoaded { kind, key }),
        Some(error) => world.trigger(AssetLoadFailed { kind, key, error }),
    }
}

/// Applies this tick's audio replies to [`LoadedAssets`] and settles their
/// loads, in the order the audio thread sent them. Runs after
/// `update_bevy_audio_messages`, so each reply is read once.
pub fn track_audio_asset_replies(mut reader: MessageReader<AudioMessage>, mut commands: Commands) {
    for msg in reader.read() {
        let change = msg.asset_change();
        let outcome = msg.load_reply();
        if change.is_none() && outcome.is_none() {
            continue;
        }
        commands.queue(move |world: &mut World| {
            if let Some(change) = change {
                world.resource_mut::<LoadedAssets>().apply(change);
            }
            if let Some(outcome) = outcome {
                settle_load(world, outcome);
            }
        });
    }
}
