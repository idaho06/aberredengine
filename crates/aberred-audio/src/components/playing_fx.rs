//! Marker component for a live sound-effect alias in the audio world.

use bevy_ecs::prelude::Component;

use super::handles::SoundHandle;

/// One entity per live FX alias (`LoadSoundAlias`). The alias handle is
/// `Copy`, so it's stored directly on the component -- no separate
/// entity-keyed map is needed in `AudioStore`.
#[derive(Component)]
pub struct PlayingFx {
    pub alias: SoundHandle,
    /// `AudioStore::fx` id of the source sound this alias shares sample data
    /// with. Lets `LoadFx` over a live id unload exactly that sound's aliases
    /// before unloading the source (an alias must never outlive its source).
    pub source_id: String,
}
