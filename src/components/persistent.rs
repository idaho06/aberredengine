use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Debug)]
/// Tag component used to mark entities that should persist across scene changes.
/// Entities with this component will not be despawned when switching scenes.
pub struct Persistent;
