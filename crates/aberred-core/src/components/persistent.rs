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
/// otherwise also match).
///
/// It still matches the observer entities of `Persistent` entities, so a
/// scene-cleanup sweep uses [`SceneCleanup`] instead.
pub type CleanableEntity = (Without<Persistent>, Without<bevy_ecs::resource::IsResource>);

/// What a scene switch keeps and despawns: everything [`CleanableEntity`]
/// matches is despawned, except the observers of [`Persistent`] entities
/// (`EntityCommands::observe` spawns them without `Persistent`).
#[derive(SystemParam)]
pub struct SceneCleanup<'w, 's> {
    persistent: Query<'w, 's, (Entity, Option<&'static ObservedBy>), With<Persistent>>,
}

impl SceneCleanup<'_, '_> {
    /// Queues one `despawn_all_where` sweep of every entity a scene switch
    /// removes.
    ///
    /// The keep-set (the observers of [`Persistent`] entities) is read now; the
    /// [`CleanableEntity`] filter is evaluated when the command is applied, so
    /// an entity spawned between queueing and applying is swept too.
    pub fn despawn_all(&self, commands: &mut Commands) {
        let keep: FxHashSet<Entity> = self
            .persistent
            .iter()
            .filter_map(|(_, observed_by)| observed_by)
            .flat_map(|observed_by| observed_by.get().iter().copied())
            .collect();
        commands
            .despawn_all_where::<Entity, CleanableEntity>(move |entity| !keep.contains(&entity));
    }

    /// The [`Persistent`] entities, e.g. for
    /// `WorldSignals::unregister_non_persistent_entities`.
    pub fn persistent_set(&self) -> FxHashSet<Entity> {
        self.persistent.iter().map(|(entity, _)| entity).collect()
    }
}
