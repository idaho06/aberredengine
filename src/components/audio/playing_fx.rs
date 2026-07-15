//! Marker component for a live sound-effect alias in the audio world.

use bevy_ecs::prelude::Component;

use super::handles::SoundHandle;

/// One entity per live FX alias (`LoadSoundAlias`). The alias handle is
/// `Copy`, so it's stored directly on the component -- no separate
/// entity-keyed map is needed in `AudioStore`.
#[derive(Component)]
pub struct PlayingFx {
    pub alias: SoundHandle,
}
