//! Scene registry resource for Rust-native scene management.
//!
//! [`SceneManager`] holds the registered scene names, each with its scene
//! entity, plus the name of the currently active scene and the initial scene.
//!
//! This resource is inserted automatically by `aberredengine::EngineBuilder`
//! when the developer uses `.add_scene()`.
//!
//! # Related
//!
//! - [`crate::systems::scene_dispatch`] — the systems that read this resource
//! - `aberredengine::EngineBuilder::add_scene` — builder registration

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::FxHashMap;

/// Registry of named scenes and active-scene tracking.
///
/// Inserted as an ECS resource. Systems in [`scene_dispatch`](crate::systems::scene_dispatch)
/// read/write this to validate switch targets and track which scene is active.
#[derive(Resource, Default)]
pub struct SceneManager {
    /// Registered scene names, each with its persistent scene entity (see
    /// [`insert_scene_manager`](crate::systems::scene_dispatch::insert_scene_manager)).
    scenes: FxHashMap<String, Entity>,
    /// Currently active scene name (set by `scene_switch_system`).
    pub active_scene: Option<String>,
    /// Initial scene name (set by `EngineBuilder`).
    pub initial_scene: Option<String>,
    /// Scene active during `Setup` while assets load (`EngineBuilder::loading_scene`).
    /// During `Setup` it is the only switch target.
    pub loading_scene: Option<String>,
}

impl SceneManager {
    /// Create an empty scene manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a scene under the given name, standing for `entity`.
    pub(crate) fn insert(&mut self, name: impl Into<String>, entity: Entity) {
        self.scenes.insert(name.into(), entity);
    }

    /// The persistent entity standing for the scene `name`, which
    /// [`SceneEntered`](crate::events::scene::SceneEntered)/[`SceneExited`](crate::events::scene::SceneExited)
    /// target. `None` for an unregistered name.
    pub fn scene_entity(&self, name: &str) -> Option<Entity> {
        self.scenes.get(name).copied()
    }

    /// Returns a sorted list of registered scene names (for error messages).
    pub fn scene_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.scenes.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// Returns how many scenes are registered.
    pub fn len(&self) -> usize {
        self.scenes.len()
    }

    /// Returns true if no scenes are registered.
    pub fn is_empty(&self) -> bool {
        self.scenes.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scene_names_sorted() {
        let mut sm = SceneManager::new();
        sm.insert("level2", Entity::PLACEHOLDER);
        sm.insert("menu", Entity::PLACEHOLDER);
        sm.insert("level1", Entity::PLACEHOLDER);
        let names = sm.scene_names();
        assert_eq!(names, vec!["level1", "level2", "menu"]);
    }
}
