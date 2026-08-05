//! Render-side clone of the scene-descriptor table.

use ::imgui::Ui as ImguiUi;
use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::resources::appstate::AppState;
use crate::resources::render::fontstore::FontStore;
use crate::resources::render::texturestore::TextureStore;
use crate::resources::signal_intents::SignalIntents;
use crate::resources::worldsignals::SignalSnapshot;
use crate::systems::scene_dispatch::WorldDrawCallback;

/// Called every frame to draw the scene's ImGui GUI.
///
/// Receives the ImGui [`Ui`](ImguiUi) handle for drawing widgets, a read-only
/// [`SignalSnapshot`] for reading current signal state, a mutable
/// [`SignalIntents`] buffer for queuing writes back to game logic, read-only
/// access to the [`TextureStore`] for displaying texture previews, read-only
/// access to the [`FontStore`] for displaying font previews, and read-only
/// access to [`AppState`] for typed Rust objects published by ECS observers.
///
/// # Contract
/// - Called from inside the render system's ImGui frame — after the game world
///   is drawn, at window resolution (not render-target resolution).
/// - Called whether or not debug mode (F11) is active.
/// - Interaction results must be communicated via [`SignalIntents`] (action flags,
///   pending edit values); queued intents are applied to `WorldSignals` at the top
///   of the next sim tick by `apply_signal_intents` (`SimSet::ApplyIntents`) —
///   one tick of latency, since this callback holds no live `&mut WorldSignals`.
///   `AppState` is read-only from the GUI's perspective — it's a snapshot clone,
///   not the live resource.
/// - `TextureStore` and `FontStore` are read-only; mutations go through observer events.
pub type GuiCallback =
    fn(&ImguiUi, &SignalSnapshot, &mut SignalIntents, &TextureStore, &FontStore, &AppState);

/// Render-side half of a scene's callbacks — the counterpart to core's
/// `SceneLogic`, joined by scene name in the facade's combined
/// `SceneDescriptor`.
#[derive(Clone)]
pub struct SceneRender {
    /// Called every frame to draw ImGui GUI widgets (optional). Rust-only.
    pub gui_callback: Option<GuiCallback>,
    /// Called every frame inside `begin_mode2D` to draw world-space overlays.
    pub world_draw_callback: Option<WorldDrawCallback>,
}

/// Render-side clone of the scene-descriptor table.
///
/// `render_system` resolves the active scene's `gui_callback`/
/// `world_draw_callback` against this table using
/// `DrawableSnapshot.active_scene`, since the live
/// [`SceneManager`](crate::resources::scenemanager::SceneManager) (with its
/// mutable `active_scene` tracking) is logic-world-only. `SceneRender` is
/// all fn pointers, so the clone is cheap and the table is immutable after
/// startup. Only inserted when the game uses `.add_scene()` — mirror of
/// `SceneManager`'s own conditional insertion.
#[derive(Resource, Default)]
pub struct RenderSceneTable(pub FxHashMap<String, SceneRender>);

impl RenderSceneTable {
    /// Look up a scene's render callbacks by name.
    pub fn get(&self, name: &str) -> Option<&SceneRender> {
        self.0.get(name)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::appstate::AppState;
    use crate::resources::signal_intents::SignalIntents;
    use crate::resources::worldsignals::SignalSnapshot;

    fn my_gui(
        _ui: &ImguiUi,
        _signals: &SignalSnapshot,
        _intents: &mut SignalIntents,
        _textures: &TextureStore,
        _fonts: &FontStore,
        _app_state: &AppState,
    ) {
    }

    #[test]
    fn gui_callback_none_by_default_intent() {
        let render = SceneRender {
            gui_callback: None,
            world_draw_callback: None,
        };
        assert!(render.gui_callback.is_none());
    }

    #[test]
    fn gui_callback_some_stores_fn_pointer() {
        let render = SceneRender {
            gui_callback: Some(my_gui),
            world_draw_callback: None,
        };
        assert!(render.gui_callback.is_some());
        assert_eq!(
            render.gui_callback.unwrap() as *const () as usize,
            my_gui as *const () as usize
        );
    }

    #[test]
    fn gui_callback_clone_preserves_fn_pointer() {
        let render = SceneRender {
            gui_callback: Some(my_gui),
            world_draw_callback: None,
        };
        let cloned = render.clone();
        assert_eq!(
            render.gui_callback.unwrap() as *const () as usize,
            cloned.gui_callback.unwrap() as *const () as usize
        );
    }
}
