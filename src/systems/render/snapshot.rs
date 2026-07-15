//! Reads the newest published `DrawableSnapshot` and reconciles it into the
//! render world's mirror entities + global-field mirror resources.

use bevy_ecs::prelude::*;

use crate::protocol::snapshot::SnapshotConsumer;
use crate::resources::render::mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};

use super::mirror::{
    reconcile_gui_buttons, reconcile_gui_labels, reconcile_gui_progress_bars,
    reconcile_gui_windows, reconcile_map_sprites, reconcile_map_texts, reconcile_screen_sprites,
    reconcile_screen_texts,
};

/// Reads the newest published snapshot off the triple buffer (latest-wins,
/// no interpolation) into a local `new_snapshot` clone (still a full clone,
/// not zero-copy). Runs after `update_bevy_render_asset_cmds`/
/// `process_render_asset_cmds`; safe because neither of those systems reads
/// or writes `DrawableSnapshot` (verified: no reference to it in
/// `src/systems/render_assets.rs`) -- only `apply_gameconfig_changes`/
/// `render_system` do, and both still run after this system either way, so
/// this frame's asset loads and this frame's snapshot are visible together.
///
/// `new_snapshot`'s 8 `Vec<...Entry>` drawable-list fields feed the 8
/// `reconcile_*` calls below (`src/systems/render/mirror.rs`), which write
/// their results into retained mirror entities rather than a resource. Its
/// 10 remaining "global" fields are each moved (by value, `new_snapshot` is
/// consumed here and never stored) into a dedicated `Render*` resource
/// (mirroring `RenderResources`/`DebugResources`'s existing param-bundling
/// pattern) -- `render_system` reads those instead of a `DrawableSnapshot`
/// resource for anything global. Unconditional, no `is_changed()`-style
/// gating, since nothing downstream needs it (`apply_gameconfig_changes`
/// does its own `Local`-based diff independently either way).
///
/// This system is exclusive (`fn(&mut World)`) because reconciling mirror
/// entities needs simultaneous spawn/despawn plus resource access, which an
/// ordinary `SystemParam` bundle can't express (same justification
/// `drain_cmds` uses in `src/systems/audio/systems.rs`) -- only `&mut World`
/// plus `ExclusiveSystemParam`s can, so the 10 global fields are fanned out
/// via explicit `world.resource_mut::<Render*>()` calls in a fixed order,
/// each either `mem::take`n or `.clone()`d depending on the field. There is
/// no render-world `DrawableSnapshot` resource to write into at the end;
/// `new_snapshot` only lives long enough to source the 8 `reconcile_*` calls
/// and the 10 global-field fan-out below, then drops.
pub fn receive_snapshot(world: &mut World) {
    let should_update = world.resource_mut::<SnapshotConsumer>().0.update();
    if !should_update {
        return;
    }

    let new_snapshot = world
        .resource_mut::<SnapshotConsumer>()
        .0
        .output_buffer()
        .clone();

    reconcile_map_sprites(world, &new_snapshot.map_sprites);
    reconcile_map_texts(world, &new_snapshot.map_texts);
    reconcile_screen_sprites(world, &new_snapshot.screen_sprites);
    reconcile_screen_texts(world, &new_snapshot.screen_texts);
    reconcile_gui_windows(world, &new_snapshot.gui_windows);
    reconcile_gui_buttons(world, &new_snapshot.gui_buttons);
    reconcile_gui_labels(world, &new_snapshot.gui_labels);
    reconcile_gui_progress_bars(world, &new_snapshot.gui_progress_bars);

    world.resource_mut::<RenderCamera>().0 = new_snapshot.camera;
    world.resource_mut::<RenderGameConfig>().0 = new_snapshot.game_config;
    world.resource_mut::<RenderSignalSnapshot>().0 = new_snapshot.signals;
    world.resource_mut::<RenderAppState>().0 = new_snapshot.app_state;
    world.resource_mut::<RenderDebugSnapshot>().0 = new_snapshot.debug;
    world.resource_mut::<RenderActiveScene>().0 = new_snapshot.active_scene;
    world.resource_mut::<RenderWorldTime>().0 = new_snapshot.world_time;
    world.resource_mut::<RenderPostProcess>().0 = new_snapshot.post_process;
    world.resource_mut::<RenderGuiThemes>().0 = new_snapshot.gui_themes;
    world.resource_mut::<RenderCameraFollow>().0 = new_snapshot.camera_follow;
}
