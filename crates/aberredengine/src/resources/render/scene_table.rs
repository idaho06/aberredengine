//! Render-side clone of the scene-descriptor table.

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::systems::scene_dispatch::SceneDescriptor;

/// Render-side clone of the scene-descriptor table.
///
/// `render_system` resolves the active scene's `gui_callback`/
/// `world_draw_callback` against this table using
/// `DrawableSnapshot.active_scene`, since the live
/// [`SceneManager`](crate::resources::scenemanager::SceneManager) (with its
/// mutable `active_scene` tracking) is logic-world-only. `SceneDescriptor` is
/// all fn pointers, so the clone is cheap and the table is immutable after
/// startup. Only inserted when the game uses `.add_scene()` — mirror of
/// `SceneManager`'s own conditional insertion.
#[derive(Resource, Default)]
pub struct RenderSceneTable(pub FxHashMap<String, SceneDescriptor>);

impl RenderSceneTable {
    /// Look up a scene descriptor by name.
    pub fn get(&self, name: &str) -> Option<&SceneDescriptor> {
        self.0.get(name)
    }
}
