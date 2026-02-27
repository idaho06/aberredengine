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

use bevy_ecs::prelude::*;
use log::{error, info};

use crate::components::persistent::Persistent;
use crate::events::audio::AudioCmd;
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::scenemanager::SceneManager;
use crate::resources::systemsstore::SystemsStore;
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
// scene_switch_system — engine-owned scene transition
// ---------------------------------------------------------------------------

/// Handles scene transitions for [`SceneManager`]-based games.
///
/// This system is registered into [`SystemsStore`] under `"switch_scene"` when
/// the developer uses [`EngineBuilder::add_scene`](crate::engine_app::EngineBuilder::add_scene).
///
/// Flow:
/// 1. Stop all music
/// 2. Despawn all non-[`Persistent`] entities
/// 3. Clear tracked groups and group counts
/// 4. Read `WorldSignals["scene"]` for the target scene name
/// 5. Call `on_exit` on the previous scene (if any)
/// 6. Update `SceneManager.active_scene`
/// 7. Call `on_enter` on the new scene
pub fn scene_switch_system(
    mut ctx: GameCtx,
    entities_to_clean: Query<Entity, Without<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut scene_manager: ResMut<SceneManager>,
) {
    info!("scene_switch_system: System called!");

    ctx.audio.write(AudioCmd::StopAllMusic);

    for entity in entities_to_clean.iter() {
        ctx.commands.entity(entity).despawn();
    }

    tracked_groups.clear();
    ctx.world_signals.clear_group_counts();

    let scene_name = ctx
        .world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or_else(|| "menu".to_string());

    // Call on_exit for the previous scene
    if let Some(prev_name) = scene_manager.active_scene.clone()
        && let Some(descriptor) = scene_manager.get(&prev_name)
        && let Some(on_exit) = descriptor.on_exit
    {
        on_exit(&mut ctx);
    }

    // Look up and call on_enter for the new scene
    if let Some(descriptor) = scene_manager.get(&scene_name) {
        let on_enter = descriptor.on_enter;
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
    if world_signals.has_flag("switch_scene") {
        world_signals.clear_flag("switch_scene");
        commands.run_system(
            *systems_store
                .get("switch_scene")
                .expect("switch_scene system not found"),
        );
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
        .expect("SceneManager.initial_scene must be set")
        .clone();

    world_signals.set_string("scene", initial);

    commands.run_system(
        *systems_store
            .get("switch_scene")
            .expect("switch_scene system not found"),
    );
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
        };
        assert!(desc.on_update.is_none());
        assert!(desc.on_exit.is_none());
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
        };
        let cloned = desc.clone();
        // fn pointers are Copy — both point to the same function
        assert_eq!(
            desc.on_enter as *const () as usize,
            cloned.on_enter as *const () as usize
        );
    }
}
