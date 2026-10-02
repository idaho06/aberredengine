//! Persistent entity marker component.
//!
//! Entities with the [`Persistent`] component will not be despawned when
//! switching scenes. Use this for global state, audio controllers, or any
//! entity that must survive scene transitions.

use bevy_ecs::observer::ObservedBy;
use bevy_ecs::prelude::{Commands, Component, Entity, Query, With, Without};
use bevy_ecs::system::SystemParam;
use rustc_hash::FxHashSet;

/// Tag component used to mark entities that should persist across scene changes.
///
/// Entities with this component will not be despawned when switching scenes.
#[derive(Component, Clone, Debug)]
pub struct Persistent;

/// Query filter for entities that are not [`Persistent`] and not one of
/// bevy's resource-backed entities (which `Query<Entity, ...>` would
/// otherwise also match in bevy_ecs 0.19+).
///
/// It still matches the observer entities of `Persistent` entities, so a
/// scene-cleanup sweep uses [`SceneCleanup`] instead.
pub type CleanableEntity = (Without<Persistent>, Without<bevy_ecs::resource::IsResource>);

/// What a scene switch keeps and despawns: everything [`CleanableEntity`]
/// matches is despawned, except the observers of [`Persistent`] entities
/// (`EntityCommands::observe` spawns them without `Persistent`).
#[derive(SystemParam)]
pub struct SceneCleanup<'w, 's> {
    cleanable: Query<'w, 's, Entity, CleanableEntity>,
    persistent: Query<'w, 's, (Entity, Option<&'static ObservedBy>), With<Persistent>>,
}

impl SceneCleanup<'_, '_> {
    /// Queues a `try_despawn` for every entity a scene switch removes.
    pub fn despawn_all(&self, commands: &mut Commands) {
        let keep: FxHashSet<Entity> = self
            .persistent
            .iter()
            .filter_map(|(_, observed_by)| observed_by)
            .flat_map(|observed_by| observed_by.get().iter().copied())
            .collect();
        for entity in self
            .cleanable
            .iter()
            .filter(|entity| !keep.contains(entity))
        {
            commands.entity(entity).try_despawn();
        }
    }

    /// The [`Persistent`] entities, e.g. for
    /// `WorldSignals::clear_non_persistent_entities`.
    pub fn persistent_set(&self) -> FxHashSet<Entity> {
        self.persistent.iter().map(|(entity, _)| entity).collect()
    }
}
