//! Persistent entity marker component.
//!
//! Entities with the [`Persistent`] component will not be despawned when
//! switching scenes. Use this for global state, audio controllers, or any
//! entity that must survive scene transitions.

use bevy_ecs::prelude::Component;

/// Tag component used to mark entities that should persist across scene changes.
///
/// Entities with this component will not be despawned when switching scenes.
#[derive(Component, Clone, Debug)]
pub struct Persistent;
