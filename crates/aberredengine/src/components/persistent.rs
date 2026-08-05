//! Persistent entity marker component.
//!
//! Entities with the [`Persistent`] component will not be despawned when
//! switching scenes. Use this for global state, audio controllers, or any
//! entity that must survive scene transitions.

use bevy_ecs::prelude::{Component, Without};

/// Tag component used to mark entities that should persist across scene changes.
///
/// Entities with this component will not be despawned when switching scenes.
#[derive(Component, Clone, Debug)]
pub struct Persistent;

/// Query filter for entities eligible for scene-cleanup/quit despawn: not
/// [`Persistent`], and not one of bevy's resource-backed entities (which
/// `Query<Entity, ...>` would otherwise also match in bevy_ecs 0.19+).
pub type CleanableEntity = (Without<Persistent>, Without<bevy_ecs::resource::IsResource>);
