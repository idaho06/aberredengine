//! Combined per-scene callback descriptor — the facade-level type that
//! joins core's `SceneLogic` and render's `SceneRender` back together for
//! `EngineBuilder::add_scene`'s public API.
//!
//! Core cannot host this struct: `gui_callback` names `ImguiUi`/
//! `TextureStore`/`FontStore`, none of which `aberred-core` can see. Splits
//! into its two halves at [`EngineBuilder::add_scene`](super::EngineBuilder::add_scene)
//! registration time, joined again by scene name.

use crate::resources::render::scene_table::GuiCallback;
use aberred_core::systems::scene_dispatch::{SceneEnterFn, SceneExitFn, SceneUpdateFn, WorldDrawCallback};

/// Describes the callbacks for a single scene.
///
/// Register one per scene name via [`EngineBuilder::add_scene`](super::EngineBuilder::add_scene).
///
/// # Example
///
/// ```ignore
/// SceneDescriptor {
///     on_enter:     menu::setup,
///     on_update:    Some(menu::update),
///     on_exit:      None,
///     gui_callback: None,
///     world_draw_callback: None,
/// }
/// ```
#[derive(Clone)]
pub struct SceneDescriptor {
    /// Called once when the scene becomes active.
    pub on_enter: SceneEnterFn,
    /// Called every frame while the scene is active (optional).
    pub on_update: Option<SceneUpdateFn>,
    /// Called once when leaving the scene (optional).
    pub on_exit: Option<SceneExitFn>,
    /// Called every frame to draw ImGui GUI widgets (optional). Rust-only.
    ///
    /// See [`GuiCallback`] for the full contract.
    pub gui_callback: Option<GuiCallback>,
    /// Called every frame inside `begin_mode2D` to draw world-space overlays.
    pub world_draw_callback: Option<WorldDrawCallback>,
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::resources::appstate::AppState;
    use aberred_core::resources::signal_intents::SignalIntents;
    use aberred_core::resources::worldsignals::SignalSnapshot;
    use crate::resources::render::fontstore::FontStore;
    use crate::resources::render::texturestore::TextureStore;
    use aberred_core::systems::GameCtx;

    #[test]
    fn scene_descriptor_default_optionals() {
        fn dummy_enter(_ctx: &mut GameCtx) {}
        let desc = SceneDescriptor {
            on_enter: dummy_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
            world_draw_callback: None,
        };
        assert!(desc.on_update.is_none());
        assert!(desc.on_exit.is_none());
        assert!(desc.world_draw_callback.is_none());
    }

    #[test]
    fn scene_descriptor_clone() {
        fn enter(_ctx: &mut GameCtx) {}
        fn my_gui(
            _ui: &::imgui::Ui,
            _signals: &SignalSnapshot,
            _intents: &mut SignalIntents,
            _textures: &TextureStore,
            _fonts: &FontStore,
            _app_state: &AppState,
        ) {
        }
        let desc = SceneDescriptor {
            on_enter: enter,
            on_update: None,
            on_exit: None,
            gui_callback: Some(my_gui),
            world_draw_callback: None,
        };
        let cloned = desc.clone();
        assert_eq!(
            desc.on_enter as *const () as usize,
            cloned.on_enter as *const () as usize
        );
        assert_eq!(
            desc.gui_callback.unwrap() as *const () as usize,
            cloned.gui_callback.unwrap() as *const () as usize
        );
    }
}
