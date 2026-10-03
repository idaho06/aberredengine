//! Render-side clone of the scene-descriptor table.

use ::imgui::Ui as ImguiUi;
use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

use crate::resources::fontstore::FontStore;
use crate::resources::texturestore::TextureStore;
use aberred_core::resources::appstate::AppState;
use aberred_core::resources::signal_intents::SignalIntents;
use aberred_core::resources::worldsignals::SignalSnapshot;
use aberred_core::systems::scene_dispatch::WorldDrawCallback;

/// Called every frame to draw the scene's ImGui GUI.
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
pub type GuiCallback = fn(&mut GuiCtx);

/// What a [`GuiCallback`] can draw with, read, and write.
///
/// New fields can be added without breaking callbacks, since only the engine
/// constructs this.
#[non_exhaustive]
pub struct GuiCtx<'a> {
    /// The ImGui handle for drawing widgets.
    pub ui: &'a ImguiUi,
    /// The latest snapshot of the logic world's signals.
    pub signals: &'a SignalSnapshot,
    /// Queued writes back to `WorldSignals`, applied at the start of the next sim tick.
    pub intents: &'a mut SignalIntents,
    /// Loaded textures, e.g. for image previews.
    pub textures: &'a TextureStore,
    /// Loaded fonts, e.g. for font previews.
    pub fonts: &'a FontStore,
    /// The latest snapshot of the logic world's [`AppState`].
    pub app_state: &'a AppState,
}

impl<'a> GuiCtx<'a> {
    pub(crate) fn new(
        ui: &'a ImguiUi,
        signals: &'a SignalSnapshot,
        intents: &'a mut SignalIntents,
        textures: &'a TextureStore,
        fonts: &'a FontStore,
        app_state: &'a AppState,
    ) -> Self {
        Self {
            ui,
            signals,
            intents,
            textures,
            fonts,
            app_state,
        }
    }
}

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
/// [`SceneManager`](aberred_core::resources::scenemanager::SceneManager) (with its
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
