//! Reads the newest published `DrawableSnapshot` and reconciles it into the
//! render world's mirror entities + global-field mirror resources.

use std::sync::Arc;

use bevy_ecs::prelude::*;

use aberred_core::protocol::snapshot::SnapshotConsumer;
use crate::resources::mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};

use super::math::camera2d_to_raylib;
use super::mirror::{
    reconcile_gui_buttons, reconcile_gui_labels, reconcile_gui_progress_bars,
    reconcile_gui_windows, reconcile_map_sprites, reconcile_map_texts, reconcile_screen_sprites,
    reconcile_screen_texts,
};

/// Reads the newest published snapshot off the triple buffer (latest-wins,
/// no interpolation) and reconciles it in place -- no clone of the snapshot
/// is taken. Runs after `update_bevy_render_asset_cmds`/
/// `process_render_asset_cmds`; safe because neither of those systems reads
/// or writes `DrawableSnapshot` (verified: no reference to it in
/// `src/systems/render_assets.rs`) -- only `apply_gameconfig_changes`/
/// `render_system` do, and both still run after this system either way, so
/// this frame's asset loads and this frame's snapshot are visible together.
///
/// The snapshot's 8 `Vec<...Entry>` drawable-list fields feed the 8
/// `reconcile_*` calls below (`src/systems/mirror.rs`), which write
/// their results into retained mirror entities rather than a resource --
/// read by reference, never cloned. Only the 10 "global" fields below are
/// actually kept, and each of those is copied/cloned into a
/// dedicated `Render*` resource (mirroring `RenderResources`/
/// `DebugResources`'s existing param-bundling pattern) -- `render_system`
/// reads those instead of a `DrawableSnapshot` resource for anything
/// global. Unconditional, no `is_changed()`-style gating, since nothing
/// downstream needs it (`apply_gameconfig_changes` does its own
/// `Local`-based diff independently either way).
///
/// This system is exclusive (`fn(&mut World)`) because reconciling mirror
/// entities needs simultaneous spawn/despawn plus resource access, which an
/// ordinary `SystemParam` bundle can't express (same justification
/// `drain_cmds` uses in `src/systems/audio/systems.rs`) -- only `&mut World`
/// plus `ExclusiveSystemParam`s can. The snapshot itself is read through a
/// `resource_scope::<SnapshotConsumer, _>` so `world` stays mutable for the
/// reconcile/global-fan-out calls while the triple buffer's output stays
/// borrowed (not moved) for the duration; the 8 `reconcile_*` functions
/// internally scope a *different* resource (`SimIdMap`), so there's no
/// reentrant-borrow conflict with the outer `SnapshotConsumer` scope. There
/// is no render-world `DrawableSnapshot` resource to write into at the end.
pub fn receive_snapshot(world: &mut World) {
    world.resource_scope::<SnapshotConsumer, _>(|world, mut consumer| {
        if !consumer.0.update() {
            return;
        }
        let snap = consumer.0.output_buffer();

        reconcile_map_sprites(world, &snap.map_sprites);
        reconcile_map_texts(world, &snap.map_texts);
        reconcile_screen_sprites(world, &snap.screen_sprites);
        reconcile_screen_texts(world, &snap.screen_texts);
        reconcile_gui_windows(world, &snap.gui_windows);
        reconcile_gui_buttons(world, &snap.gui_buttons);
        reconcile_gui_labels(world, &snap.gui_labels);
        reconcile_gui_progress_bars(world, &snap.gui_progress_bars);

        world.resource_mut::<RenderCamera>().0 = camera2d_to_raylib(snap.camera);
        world
            .resource_mut::<RenderGameConfig>()
            .0
            .clone_from(&snap.game_config);
        world.resource_mut::<RenderSignalSnapshot>().0 = Arc::clone(&snap.signals);
        world
            .resource_mut::<RenderAppState>()
            .0
            .clone_from(&snap.app_state);
        world
            .resource_mut::<RenderDebugSnapshot>()
            .0
            .clone_from(&snap.debug);
        world.resource_mut::<RenderActiveScene>().0 = snap.active_scene.clone();
        world.resource_mut::<RenderWorldTime>().0 = snap.world_time;
        world
            .resource_mut::<RenderPostProcess>()
            .0
            .clone_from(&snap.post_process);
        world
            .resource_mut::<RenderGuiThemes>()
            .0
            .clone_from(&snap.gui_themes);
        world
            .resource_mut::<RenderCameraFollow>()
            .0
            .clone_from(&snap.camera_follow);
    });
}
