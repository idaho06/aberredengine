//! Scene dispatch systems for Rust-native scene management.
//!
//! This module provides systems and types for the [`SceneManager`]
//! pattern — an optional higher-level alternative to the raw `.on_switch_scene()` hook.
//!
//! - [`SceneLogic`] — per-scene logic callbacks (`on_enter`, `on_update`, `on_exit`);
//!   the render-side half (`gui_callback`/`world_draw_callback`) lives in
//!   `aberred-render`'s `SceneRender`, joined by scene name in the facade's
//!   combined `SceneDescriptor`.
//! - [`scene_switch_system`] — engine-owned scene transition: despawn → on_exit → on_enter
//! - [`scene_update_system`] — per-frame dispatch to the active scene's `on_update`
//! - [`scene_switch_poll`] — polls `WorldSignals["switch_scene"]` and triggers a scene transition
//! - [`scene_enter_play`] — one-shot system that seeds the initial scene and triggers the first switch
//!
//! Callbacks receive `&mut `[`GameCtx`] for full ECS access.
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
//! - `aberredengine::EngineBuilder::add_scene` — builder method for registration

use bevy_ecs::prelude::*;
use log::{debug, error, info};
use rustc_hash::FxHashSet;

use crate::components::persistent::{CleanableEntity, Persistent};
use crate::math::{Color, Vec2};
use crate::resources::appstate::AppState;
use crate::resources::camera2d::Camera2D;
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_keys as sk;
use crate::resources::systemsstore as hook_keys;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::{SignalSnapshot, WorldSignals};
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

/// Minimal world-space drawing interface for `WorldDrawCallback`.
/// Uses concrete types only so the callback stays object-safe.
pub trait WorldDraw {
    fn draw_line_v(&mut self, start: Vec2, end: Vec2, color: Color);
    fn draw_line_ex(&mut self, start_pos: Vec2, end_pos: Vec2, thick: f32, color: Color);
    fn draw_line_dashed(
        &mut self,
        start_pos: Vec2,
        end_pos: Vec2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    );
    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color);
}

// The `WorldDraw` impl over raylib draw handles lives in
// `aberred-render`'s `systems::math` as `RaylibWorldDraw` — a newtype
// wrapper, not a blanket impl, since `aberred-render` owns neither
// `WorldDraw` (defined here, in core) nor `RaylibDraw` (defined in the
// `raylib` crate) — the orphan rule blocks a blanket impl across two
// foreign types.

// `GuiCallback` (the ImGui-drawing callback type) lives in
// `aberred-render`'s `resources::scene_table` alongside `SceneRender` —
// core cannot name `ImguiUi`/`TextureStore`/`FontStore`.

/// Called every frame inside `begin_mode2D` in camera-transformed world space.
///
/// The [`SignalSnapshot`] param is read-only, mirroring the render-side
/// `GuiCallback`, which likewise takes no live `&WorldSignals`.
pub type WorldDrawCallback =
    fn(&mut dyn WorldDraw, &Camera2D, &ScreenSize, &AppState, &SignalSnapshot);

// ---------------------------------------------------------------------------
// SceneLogic
// ---------------------------------------------------------------------------

/// Logic-side callbacks for a single scene (core-only half of the combined
/// `SceneDescriptor` the facade exposes via `EngineBuilder::add_scene`).
#[derive(Clone)]
pub struct SceneLogic {
    /// Called once when the scene becomes active.
    pub on_enter: SceneEnterFn,
    /// Called every frame while the scene is active (optional).
    pub on_update: Option<SceneUpdateFn>,
    /// Called once when leaving the scene (optional).
    pub on_exit: Option<SceneExitFn>,
}

// ---------------------------------------------------------------------------
// scene_switch_system — engine-owned scene transition
// ---------------------------------------------------------------------------

/// Handles scene transitions for [`SceneManager`]-based games.
///
/// This system is registered into [`SystemsStore`] under `"switch_scene"` when
/// the developer uses `aberredengine::EngineBuilder::add_scene`.
///
/// Reads the target from `WorldSignals["scene"]` and checks it first: an
/// unregistered name logs an error and leaves the current scene untouched.
/// Otherwise it despawns non-[`Persistent`] entities, clears tracked groups,
/// runs the old scene's `on_exit`, records `WorldSignals["previous_scene"]`,
/// and enters the new scene.
pub fn scene_switch_system(
    mut ctx: GameCtx,
    entities_to_clean: Query<Entity, CleanableEntity>,
    persistent_entities: Query<Entity, With<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut scene_manager: ResMut<SceneManager>,
) {
    debug!("scene_switch_system: System called!");

    let scene_name = ctx
        .world_signals
        .get_string(sk::SCENE)
        .cloned()
        .unwrap_or_else(|| sk::DEFAULT_SCENE.to_string());
    let Some(on_enter) = scene_manager.get(&scene_name).map(|scene| scene.on_enter) else {
        error!(
            "scene_switch_system: No scene registered for '{}'; staying in the current scene. Registered scenes: {:?}",
            scene_name,
            scene_manager.scene_names()
        );
        return;
    };

    for entity in entities_to_clean.iter() {
        ctx.commands.entity(entity).try_despawn();
    }

    // Clear entity registrations for despawned (non-persistent) entities
    let persistent_set: FxHashSet<Entity> = persistent_entities.iter().collect();
    ctx.world_signals
        .clear_non_persistent_entities(&persistent_set);

    tracked_groups.clear();
    ctx.world_signals.clear_group_counts();

    if let Some(prev) = scene_manager.active_scene.take() {
        if let Some(on_exit) = scene_manager.get(&prev).and_then(|scene| scene.on_exit) {
            on_exit(&mut ctx);
        }
        ctx.world_signals.set_string("previous_scene", prev);
    }

    info!("scene_switch_system: Entering scene '{}'", scene_name);
    scene_manager.active_scene = Some(scene_name);
    on_enter(&mut ctx);
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
/// `aberredengine::EngineBuilder::add_scene()`.
pub fn scene_switch_poll(
    mut commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
) {
    if world_signals.take_flag(sk::SWITCH_SCENE) {
        commands.run_system(*systems_store.get(hook_keys::SWITCH_SCENE).expect("'switch_scene' system not registered; validate_required_systems should have caught this"));
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

    commands.run_system(*systems_store.get(hook_keys::SWITCH_SCENE).expect(
        "'switch_scene' system not registered; validate_required_systems should have caught this",
    ));
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::insert_game_ctx_resources;
    use bevy_ecs::system::RunSystemOnce;

    const EXITED: &str = "test_menu_exited";
    const ENTERED: &str = "test_level_entered";

    fn menu_exit(ctx: &mut GameCtx) {
        ctx.world_signals.set_flag(EXITED);
    }

    fn level_enter(ctx: &mut GameCtx) {
        ctx.world_signals.set_flag(ENTERED);
    }

    /// World with `menu` active (one scene entity, one tracked group) and
    /// `WorldSignals[scene]` set to `target`.
    fn world_in_menu_switching_to(target: &str) -> (World, Entity) {
        let mut world = World::new();
        insert_game_ctx_resources(&mut world);
        let logic = |on_enter, on_exit| SceneLogic {
            on_enter,
            on_update: None,
            on_exit,
        };
        let mut scene_manager = SceneManager::new();
        scene_manager.insert("menu", logic(|_| {}, Some(menu_exit)));
        scene_manager.insert("level", logic(level_enter, None));
        scene_manager.active_scene = Some("menu".to_owned());
        world.insert_resource(scene_manager);
        let mut groups = TrackedGroups::default();
        groups.add_group("enemies");
        world.insert_resource(groups);
        world
            .resource_mut::<WorldSignals>()
            .set_string(sk::SCENE, target);
        let menu_entity = world.spawn_empty().id();
        (world, menu_entity)
    }

    #[test]
    fn switch_to_a_registered_scene_tears_down_and_enters() {
        let (mut world, menu_entity) = world_in_menu_switching_to("level");
        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.get_entity(menu_entity).is_err());
        let signals = world.resource::<WorldSignals>();
        assert!(signals.has_flag(EXITED) && signals.has_flag(ENTERED));
        assert_eq!(
            world.resource::<SceneManager>().active_scene.as_deref(),
            Some("level")
        );
    }

    #[test]
    fn switch_to_an_unknown_scene_keeps_the_current_scene() {
        let (mut world, menu_entity) = world_in_menu_switching_to("nope");
        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.get_entity(menu_entity).is_ok(), "entities kept");
        assert!(
            !world.resource::<WorldSignals>().has_flag(EXITED),
            "on_exit not called"
        );
        assert!(world.resource::<TrackedGroups>().has_group("enemies"));
        assert_eq!(
            world.resource::<SceneManager>().active_scene.as_deref(),
            Some("menu")
        );
    }
}
