//! Component for a currently playing-or-paused music track in the audio world.

use bevy_ecs::prelude::Component;

use super::handles::MusicHandle;

/// One entity per music id that is currently playing-or-paused. A loaded
/// but not (yet) playing track has no `MusicTrack` entity -- it lives only
/// in `AudioStore::music` (any number of tracks can be loaded; only
/// playing/paused ones are pumped). The stream handle is cached here
/// (rather than re-looked-up
/// from `AudioStore::music` by `id` every tick) since `pump_music` runs
/// every audio tick regardless of commands.
#[derive(Component)]
pub struct MusicTrack {
    pub id: String,
    pub music: MusicHandle,
    pub looped: bool,
    pub paused: bool,
}
