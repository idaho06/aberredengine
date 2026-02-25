//! Scene registry resource for Rust-native scene management.
//!
//! [`SceneManager`] holds a registry of [`SceneDescriptor`]s keyed by scene
//! name, plus the name of the currently active scene and the initial scene.
//!
//! This resource is inserted automatically by [`EngineBuilder`](crate::engine_app::EngineBuilder)
//! when the developer uses `.add_scene()`.
//!
//! # Related
//!
//! - [`crate::systems::scene_dispatch`] — the systems that read this resource
//! - [`crate::engine_app::EngineBuilder::add_scene`] — builder registration

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::systems::scene_dispatch::SceneDescriptor;

/// Registry of named scenes and active-scene tracking.
///
/// Inserted as an ECS resource. Systems in [`scene_dispatch`](crate::systems::scene_dispatch)
/// read/write this to look up callbacks and track which scene is active.
#[derive(Resource)]
pub struct SceneManager {
    scenes: FxHashMap<String, SceneDescriptor>,
    /// Currently active scene name (set by `scene_switch_system`).
    pub active_scene: Option<String>,
    /// Initial scene name (set by `EngineBuilder`).
    pub initial_scene: Option<String>,
}

impl SceneManager {
    /// Create an empty scene manager.
    pub fn new() -> Self {
        Self {
            scenes: FxHashMap::default(),
            active_scene: None,
            initial_scene: None,
        }
    }

    /// Register a scene under the given name.
    pub fn insert(&mut self, name: impl Into<String>, descriptor: SceneDescriptor) {
        self.scenes.insert(name.into(), descriptor);
    }

    /// Look up a scene descriptor by name.
    pub fn get(&self, name: &str) -> Option<&SceneDescriptor> {
        self.scenes.get(name)
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

impl Default for SceneManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::scene_dispatch::{SceneCtx, SceneDescriptor};

    fn dummy_enter(_ctx: &mut SceneCtx) {}

    fn make_descriptor() -> SceneDescriptor {
        SceneDescriptor {
            on_enter: dummy_enter,
            on_update: None,
            on_exit: None,
        }
    }

    #[test]
    fn new_is_empty() {
        let sm = SceneManager::new();
        assert!(sm.is_empty());
        assert_eq!(sm.len(), 0);
        assert!(sm.active_scene.is_none());
        assert!(sm.initial_scene.is_none());
    }

    #[test]
    fn insert_and_get() {
        let mut sm = SceneManager::new();
        sm.insert("menu", make_descriptor());
        assert_eq!(sm.len(), 1);
        assert!(sm.get("menu").is_some());
        assert!(sm.get("nonexistent").is_none());
    }

    #[test]
    fn scene_names_sorted() {
        let mut sm = SceneManager::new();
        sm.insert("level2", make_descriptor());
        sm.insert("menu", make_descriptor());
        sm.insert("level1", make_descriptor());
        let names = sm.scene_names();
        assert_eq!(names, vec!["level1", "level2", "menu"]);
    }

    #[test]
    fn insert_overwrites() {
        fn other_enter(_ctx: &mut SceneCtx) {}
        let mut sm = SceneManager::new();
        sm.insert("menu", make_descriptor());
        sm.insert(
            "menu",
            SceneDescriptor {
                on_enter: other_enter,
                on_update: None,
                on_exit: None,
            },
        );
        assert_eq!(sm.len(), 1);
        let desc = sm.get("menu").unwrap();
        assert_eq!(desc.on_enter as *const () as usize, other_enter as *const () as usize);
    }

    #[test]
    fn default_is_empty() {
        let sm = SceneManager::default();
        assert!(sm.is_empty());
    }
}
