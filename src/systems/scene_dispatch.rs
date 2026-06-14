//! Scene dispatch systems for Rust-native scene management.
//!
//! This module provides systems and types for the [`SceneManager`](crate::resources::scenemanager::SceneManager)
//! pattern — an optional higher-level alternative to the raw `.on_switch_scene()` hook.
//!
//! - [`SceneDescriptor`] — per-scene callbacks (`on_enter`, `on_update`, `on_exit`)
//! - [`scene_switch_system`] — engine-owned scene transition: despawn → on_exit → on_enter
//! - [`scene_update_system`] — per-frame dispatch to the active scene's `on_update`
//! - [`scene_switch_poll`] — polls `WorldSignals["switch_scene"]` and triggers a scene transition
//! - [`scene_enter_play`] — one-shot system that seeds the initial scene and triggers the first switch
//!
//! Callbacks receive `&mut `[`GameCtx`](crate::systems::GameCtx) for full ECS access.
//!
//! # Callback Signatures
//!
//! ```ignore
//! fn my_enter(ctx: &mut GameCtx) { /* spawn scene entities */ }
//! fn my_update(ctx: &mut GameCtx, dt: f32, input: &InputState) { /* per-frame logic */ }
//! fn my_exit(ctx: &mut GameCtx) { /* cleanup before leaving */ }
//! ```
//!
//! # Related
//!
//! - [`crate::resources::scenemanager::SceneManager`] — the registry resource
//! - [`crate::engine_app::EngineBuilder::add_scene`] — builder method for registration

use ::imgui::Ui as ImguiUi;
use bevy_ecs::prelude::*;
use log::{debug, error, info};
use raylib::prelude::{Camera2D, Color, Vector2};
use rustc_hash::FxHashSet;

use crate::components::persistent::Persistent;
use crate::resources::appstate::AppState;
use crate::resources::fontstore::FontStore;
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_keys as sk;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::GameCtx;

// ---------------------------------------------------------------------------
// Callback type aliases
// ---------------------------------------------------------------------------

/// Called when entering a scene (spawn entities, initialize state).
pub type SceneEnterFn = for<'w, 's> fn(&mut GameCtx<'w, 's>);

/// Called every frame while the scene is active. `f32` is delta time, `&InputState` is current input.
pub type SceneUpdateFn = for<'w, 's> fn(&mut GameCtx<'w, 's>, f32, &InputState);

/// Called when leaving a scene (cleanup before despawn).
pub type SceneExitFn = for<'w, 's> fn(&mut GameCtx<'w, 's>);

/// Called every frame to draw the scene's ImGui GUI.
///
/// Receives the ImGui [`Ui`](ImguiUi) handle for drawing widgets, a mutable
/// reference to [`WorldSignals`] for reading current state and writing user
/// actions back to game logic, read-only access to the [`TextureStore`]
/// for displaying texture previews, read-only access to the [`FontStore`]
/// for displaying font previews, and read-only access to [`AppState`]
/// for typed Rust objects published by ECS observers.
///
/// # Contract
/// - Called from inside the render system's ImGui frame — after the game world
///   is drawn, at window resolution (not render-target resolution).
/// - Called whether or not debug mode (F11) is active.
/// - Interaction results must be communicated via `WorldSignals` (action flags,
///   pending edit values). `AppState` is read-only from the GUI's perspective.
/// - `TextureStore` and `FontStore` are read-only; mutations go through observer events.
///
/// # Example
/// ```rust,ignore
/// fn my_gui(ui: &ImguiUi, signals: &mut WorldSignals, _textures: &TextureStore, _fonts: &FontStore, app_state: &AppState) {
///     if let Some(snap) = app_state.get::<MySnapshot>() {
///         ui.text(format!("value: {}", snap.value));
///     }
///     if ui.button("Save") {
///         signals.set_flag("gui:action:file:save");
///     }
/// }
/// ```
/// Minimal world-space drawing interface for `WorldDrawCallback`.
/// Uses concrete types only so the callback stays object-safe.
pub trait WorldDraw {
    fn draw_line_v(&mut self, start: Vector2, end: Vector2, color: Color);
    fn draw_line_ex(&mut self, start_pos: Vector2, end_pos: Vector2, thick: f32, color: Color);
    fn draw_line_dashed(
        &mut self,
        start_pos: Vector2,
        end_pos: Vector2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    );
    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color);
}

impl<T: raylib::prelude::RaylibDraw> WorldDraw for T {
    fn draw_line_v(&mut self, start: Vector2, end: Vector2, color: Color) {
        raylib::prelude::RaylibDraw::draw_line_v(self, start, end, color);
    }

    fn draw_line_ex(&mut self, start_pos: Vector2, end_pos: Vector2, thick: f32, color: Color) {
        raylib::prelude::RaylibDraw::draw_line_ex(self, start_pos, end_pos, thick, color);
    }

    fn draw_line_dashed(
        &mut self,
        start_pos: Vector2,
        end_pos: Vector2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    ) {
        raylib::prelude::RaylibDraw::draw_line_dashed(
            self, start_pos, end_pos, dash_size, space_size, color,
        );
    }

    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        raylib::prelude::RaylibDraw::draw_line(self, x1, y1, x2, y2, color);
    }
}

pub type GuiCallback = fn(&ImguiUi, &mut WorldSignals, &TextureStore, &FontStore, &AppState);

/// Called every frame inside `begin_mode2D` in camera-transformed world space.
pub type WorldDrawCallback =
    fn(&mut dyn WorldDraw, &Camera2D, &ScreenSize, &AppState, &WorldSignals);

// ---------------------------------------------------------------------------
// SceneDescriptor
// ---------------------------------------------------------------------------

/// Describes the callbacks for a single scene.
///
/// Register one per scene name via [`EngineBuilder::add_scene`](crate::engine_app::EngineBuilder::add_scene).
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
// scene_switch_system — engine-owned scene transition
// ---------------------------------------------------------------------------

/// Handles scene transitions for [`SceneManager`]-based games.
///
/// This system is registered into [`SystemsStore`] under `"switch_scene"` when
/// the developer uses [`EngineBuilder::add_scene`](crate::engine_app::EngineBuilder::add_scene).
///
/// Flow:
/// 1. Despawn all non-[`Persistent`] entities
/// 2. Clear tracked groups and group counts
/// 3. Read `WorldSignals["scene"]` for the target scene name
/// 4. Call `on_exit` on the previous scene (if any)
/// 5. Write previous scene name to `WorldSignals["previous_scene"]` (if any)
/// 6. Update `SceneManager.active_scene`
/// 7. Call `on_enter` on the new scene
pub fn scene_switch_system(
    mut ctx: GameCtx,
    entities_to_clean: Query<Entity, Without<Persistent>>,
    persistent_entities: Query<Entity, With<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut scene_manager: ResMut<SceneManager>,
) {
    debug!("scene_switch_system: System called!");

    let prev_scene = scene_manager.active_scene.clone();

    for entity in entities_to_clean.iter() {
        ctx.commands.entity(entity).try_despawn();
    }

    // Clear entity registrations for despawned (non-persistent) entities
    let persistent_set: FxHashSet<Entity> = persistent_entities.iter().collect();
    ctx.world_signals
        .clear_non_persistent_entities(&persistent_set);

    tracked_groups.clear();
    ctx.world_signals.clear_group_counts();

    let scene_name = ctx
        .world_signals
        .get_string(sk::SCENE)
        .cloned()
        .unwrap_or_else(|| sk::DEFAULT_SCENE.to_string());

    // Call on_exit for the previous scene
    if let Some(ref prev_name) = prev_scene
        && let Some(descriptor) = scene_manager.get(prev_name)
        && let Some(on_exit) = descriptor.on_exit
    {
        on_exit(&mut ctx);
    }

    // Look up and call on_enter for the new scene
    if let Some(descriptor) = scene_manager.get(&scene_name) {
        let on_enter = descriptor.on_enter;
        if let Some(ref prev) = prev_scene {
            ctx.world_signals
                .set_string("previous_scene", prev.as_str());
        }
        scene_manager.active_scene = Some(scene_name.clone());
        on_enter(&mut ctx);
        info!("scene_switch_system: Entered scene '{}'", scene_name);
    } else {
        error!(
            "scene_switch_system: No scene registered for '{}'. Registered scenes: {:?}",
            scene_name,
            scene_manager.scene_names()
        );
    }
}

// ---------------------------------------------------------------------------
// scene_update_system — per-frame dispatch
// ---------------------------------------------------------------------------

/// Calls `on_update` for the active scene each frame.
///
/// Looks up the active scene in [`SceneManager`], and if it has an `on_update`
/// callback, calls it with `(ctx, dt)`.
pub fn scene_update_system(
    mut ctx: GameCtx,
    scene_manager: Res<SceneManager>,
    world_time: Res<WorldTime>,
    input: Res<InputState>,
) {
    let dt = world_time.delta;
    if let Some(ref active_name) = scene_manager.active_scene
        && let Some(descriptor) = scene_manager.get(active_name)
        && let Some(on_update) = descriptor.on_update
    {
        on_update(&mut ctx, dt, &input);
    }
}

/// Polls the `"switch_scene"` flag in [`WorldSignals`] and runs the
/// scene switch system when set.
///
/// Added to the update schedule automatically when using
/// [`EngineBuilder::add_scene()`](crate::engine_app::EngineBuilder::add_scene).
pub fn scene_switch_poll(
    mut commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
) {
    if world_signals.take_flag(sk::SWITCH_SCENE) {
        commands.run_system(*systems_store.get("switch_scene").expect("'switch_scene' system not registered; validate_required_systems should have caught this"));
    }
}

// ---------------------------------------------------------------------------
// scene_enter_play — one-shot bootstrap
// ---------------------------------------------------------------------------

/// One-shot system registered as `"enter_play"` for SceneManager-based games.
///
/// Seeds `WorldSignals["scene"]` with the initial scene name (stored in
/// [`SceneManager`]) and then runs the `switch_scene` system.
pub fn scene_enter_play(
    mut commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
    scene_manager: Res<SceneManager>,
) {
    let initial = scene_manager
        .initial_scene
        .as_ref()
        .cloned()
        .expect("SceneManager.initial_scene not set; validate_builder should have caught this");

    world_signals.set_string(sk::SCENE, initial);

    commands.run_system(*systems_store.get("switch_scene").expect(
        "'switch_scene' system not registered; validate_required_systems should have caught this",
    ));
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::GameCtx;

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
    fn scene_descriptor_with_all_callbacks() {
        fn enter(_ctx: &mut GameCtx) {}
        fn update(_ctx: &mut GameCtx, _dt: f32, _input: &InputState) {}
        fn exit(_ctx: &mut GameCtx) {}
        let desc = SceneDescriptor {
            on_enter: enter,
            on_update: Some(update),
            on_exit: Some(exit),
            gui_callback: None,
            world_draw_callback: None,
        };
        assert!(desc.on_update.is_some());
        assert!(desc.on_exit.is_some());
    }

    #[test]
    fn scene_descriptor_clone() {
        fn enter(_ctx: &mut GameCtx) {}
        let desc = SceneDescriptor {
            on_enter: enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
            world_draw_callback: None,
        };
        let cloned = desc.clone();
        // fn pointers are Copy — both point to the same function
        assert_eq!(
            desc.on_enter as *const () as usize,
            cloned.on_enter as *const () as usize
        );
    }

    #[test]
    fn gui_callback_none_by_default_intent() {
        fn enter(_ctx: &mut GameCtx) {}
        let desc = SceneDescriptor {
            on_enter: enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
            world_draw_callback: None,
        };
        assert!(desc.gui_callback.is_none());
    }

    #[test]
    fn gui_callback_some_stores_fn_pointer() {
        fn enter(_ctx: &mut GameCtx) {}
        fn my_gui(
            _ui: &ImguiUi,
            _signals: &mut WorldSignals,
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
        assert!(desc.gui_callback.is_some());
        assert_eq!(
            desc.gui_callback.unwrap() as *const () as usize,
            my_gui as *const () as usize
        );
    }

    #[test]
    fn gui_callback_clone_preserves_fn_pointer() {
        fn enter(_ctx: &mut GameCtx) {}
        fn my_gui(
            _ui: &ImguiUi,
            _signals: &mut WorldSignals,
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
            desc.gui_callback.unwrap() as *const () as usize,
            cloned.gui_callback.unwrap() as *const () as usize
        );
    }
}
