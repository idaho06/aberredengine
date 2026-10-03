//! Per-scene logic callback descriptor for `EngineBuilder::add_scene`.
//!
//! Converted into core's `SceneLogic` at registration time. A scene's render
//! callbacks are registered separately, with `EngineBuilder::add_scene_gui` /
//! `EngineBuilder::add_scene_world_draw`.

use aberred_core::systems::scene_dispatch::{SceneEnterFn, SceneExitFn, SceneUpdateFn};

/// Describes the logic callbacks for a single scene.
///
/// Register one per scene name via [`EngineBuilder::add_scene`](super::EngineBuilder::add_scene).
///
/// # Example
///
/// ```ignore
/// SceneDescriptor {
///     on_enter:  menu::setup,
///     on_update: Some(menu::update),
///     on_exit:   None,
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
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
