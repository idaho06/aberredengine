//! Scene transition events.
//!
//! [`scene_switch_system`](crate::systems::scene_dispatch::scene_switch_system) triggers
//! [`SceneExited`] for the scene being left, then [`SceneEntered`] and [`SceneReady`] for the
//! new one. All are [`EntityEvent`]s targeted at the scene's entity (see
//! [`SceneName`](crate::components::scene::SceneName)), so they reach observers attached to
//! that entity as well as global observers.
//!
//! - [`SceneExited`] fires before the old scene is torn down: its entities and entity
//!   registrations are still alive.
//! - [`SceneEntered`] fires after the teardown, so entities spawned by its observers belong to
//!   the new scene.
//! - [`SceneReady`] fires once the [`SceneEntered`] observers' commands have applied.

use std::sync::Arc;

use bevy_ecs::prelude::*;

/// Triggered once when a scene becomes active. Global observers run before
/// observers of the scene entity, and their commands apply afterwards; observe
/// [`SceneReady`] to run after all of them.
#[derive(EntityEvent, Clone, Debug)]
pub struct SceneEntered {
    /// The entered scene's entity.
    #[event_target]
    pub scene: Entity,
    /// The entered scene's name.
    pub name: Arc<str>,
    /// The scene that was active before, if any.
    pub previous: Option<Arc<str>>,
}

/// Triggered once right after [`SceneEntered`], when every [`SceneEntered`]
/// observer has run and its commands have applied: entities those observers
/// spawned exist, and their signal writes are visible.
#[derive(EntityEvent, Clone, Debug)]
pub struct SceneReady {
    /// The entered scene's entity.
    #[event_target]
    pub scene: Entity,
    /// The entered scene's name.
    pub name: Arc<str>,
}

/// Triggered once when a scene is left, before its entities are despawned.
#[derive(EntityEvent, Clone, Debug)]
pub struct SceneExited {
    /// The exited scene's entity.
    #[event_target]
    pub scene: Entity,
    /// The exited scene's name.
    pub name: Arc<str>,
    /// The scene being switched to.
    pub next: Arc<str>,
}
