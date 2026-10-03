//! Scene transition events.
//!
//! [`scene_switch_system`](crate::systems::scene_dispatch::scene_switch_system) triggers
//! [`SceneExited`] for the scene being left, then [`SceneEntered`] for the new one. Both are
//! [`EntityEvent`]s targeted at the scene's entity (see
//! [`SceneName`](crate::components::scene::SceneName)), so they reach observers attached to
//! that entity as well as global observers.
//!
//! - [`SceneExited`] fires before the old scene is torn down: its entities and entity
//!   registrations are still alive.
//! - [`SceneEntered`] fires after the teardown, so entities spawned by its observers belong to
//!   the new scene.

use std::sync::Arc;

use bevy_ecs::prelude::*;

/// Triggered once when a scene becomes active.
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
