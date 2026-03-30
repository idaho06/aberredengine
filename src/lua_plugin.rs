//! Game-specific logic and scene management.
//!
//! This module contains the game's setup, update loop, and scene switching
//! logic. It demonstrates how to use the engine's components and systems
//! to build an Arkanoid-style game.
//!
//! # Key Functions
//!
//! - [`setup`] – loads resources (textures, fonts, audio) during `Setup` state
//! - [`enter_play`] – initializes world signals and observers when entering `Playing` state
//! - [`switch_scene`] – handles scene transitions (menu, level01, etc.)
//! - [`update`] – per-frame game logic for each scene
//!
//! # Scene Architecture
//!
//! Scenes are managed via the `"scene"` string in [`WorldSignals`](crate::resources::worldsignals::WorldSignals).
//! Setting the `"switch_scene"` flag triggers [`switch_scene`] to despawn non-persistent
//! entities and spawn the new scene's entities.
//!
//! # Phase Callbacks
//!
//! The `level01` scene uses [`Phase`](crate::components::phase::Phase) with callbacks:
//! - `init` → `get_started` → `playing` → `lose_life`/`level_cleared`/`game_over`
//!
//! These callbacks manage ball spawning, life tracking, and win/lose conditions.
//!
//! # Collision Callbacks
//!
//! [`CollisionRule`](crate::components::collision::CollisionRule) components define how
//! entities interact: ball-wall bounce, ball-player reflection, ball-brick destruction.

use crate::components::luaphase::LuaPhase;
use crate::components::persistent::Persistent;
use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationStore;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::lua_runtime::{InputSnapshot, LuaRuntime};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::tilemapstore::{Tilemap, TilemapStore};
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::lua_commands::{
    EntityCmdQueries, process_animation_command, process_asset_command, process_audio_command,
    process_camera_command, process_camera_follow_command, process_clone_command,
    process_entity_commands, process_gameconfig_command, process_group_command,
    process_input_command, process_phase_command, process_render_command, process_signal_command,
    process_spawn_command, process_tilemap_command,
};
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use log::{error, info};
use raylib::ffi;
use raylib::ffi::TextureFilter::TEXTURE_FILTER_ANISOTROPIC_8X;
use raylib::prelude::*;
use rustc_hash::FxHashSet;

/// Bundled Lua runtime + audio command writer for scripting systems.
#[derive(SystemParam)]
pub struct ScriptingContext<'w> {
    pub lua_runtime: NonSend<'w, LuaRuntime>,
    pub audio_cmd_writer: MessageWriter<'w, AudioCmd>,
}

/// Bundled game scene state resources.
#[derive(SystemParam)]
pub struct GameSceneState<'w> {
    pub world_signals: ResMut<'w, WorldSignals>,
    pub post_process: ResMut<'w, PostProcessShader>,
    pub config: ResMut<'w, GameConfig>,
    pub camera_follow: ResMut<'w, CameraFollowConfig>,
    pub systems_store: Res<'w, SystemsStore>,
    pub anim_store: Res<'w, AnimationStore>,
}

/// Bundled entity processing queries.
#[derive(SystemParam)]
pub struct EntityProcessing<'w, 's> {
    pub cmd_queries: EntityCmdQueries<'w, 's>,
    pub luaphase: Query<'w, 's, (Entity, &'static mut LuaPhase)>,
}

/// Load a font with mipmaps and anisotropic filtering
fn load_font_with_mipmaps(rl: &mut RaylibHandle, th: &RaylibThread, path: &str, size: i32) -> Font {
    let mut font = rl
        .load_font_ex(th, path, size, None)
        .unwrap_or_else(|_| panic!("Failed to load font '{}'", path));
    unsafe {
        ffi::GenTextureMipmaps(&mut font.texture);
        ffi::SetTextureFilter(font.texture, TEXTURE_FILTER_ANISOTROPIC_8X as i32);
    }
    font
}

/// Helper function to load a png and a json describing a tilemap. The json comes from Tilesetter 2.1.0
fn load_tilemap(rl: &mut RaylibHandle, thread: &RaylibThread, path: &str) -> (Texture2D, Tilemap) {
    let dirname = path.split('/').next_back().expect("Not a valid dir path.");
    let json_path = format!("{}/{}.txt", path, dirname);
    let png_path = format!("{}/{}.png", path, dirname);

    let texture = rl
        .load_texture(thread, &png_path)
        .expect("Failed to load tilemap texture");
    let json_string = std::fs::read_to_string(json_path).expect("Failed to load tilemap JSON");
    let tilemap: Tilemap =
        serde_json::from_str(&json_string).expect("Failed to parse tilemap JSON");
    (texture, tilemap)
}

// This function is meant to load all resources
pub fn setup(
    mut commands: Commands,
    mut next_state: ResMut<NextGameState>,
    mut raylib: crate::systems::RaylibAccess,
    mut fonts: NonSendMut<FontStore>,
    mut shaders: NonSendMut<ShaderStore>,
    mut scripting: ScriptingContext,
) {
    // This function sets up the game world, loading resources
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);

    // Default camera. Needed to start the engine before entering play state
    // The camera will be overridden later in the scene setup
    let camera = Camera2D {
        target: Vector2 {
            x: 0.0,
            y: 0.0, //x: 0.0,
                    //y: 0.0,
        },
        offset: Vector2 {
            x: rl.get_screen_width() as f32 * 0.5,
            y: rl.get_screen_height() as f32 * 0.5,
        },
        rotation: 0.0,
        zoom: 1.0,
    };
    commands.insert_resource(Camera2DRes(camera));

    let lua_runtime = &scripting.lua_runtime;

    // Call Lua on_setup function to queue asset loading commands
    if lua_runtime.has_function("on_setup")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_setup", ())
    {
        error!("Error calling on_setup: {}", e);
    }

    // Initialize stores
    let mut tex_store = TextureStore::new();
    let mut tilemaps_store = TilemapStore::new();

    // Process asset commands queued by Lua
    for cmd in lua_runtime.drain_asset_commands() {
        process_asset_command(
            rl,
            th,
            cmd,
            &mut tex_store,
            &mut tilemaps_store,
            &mut fonts,
            &mut shaders,
            &mut scripting.audio_cmd_writer,
            load_font_with_mipmaps,
            load_tilemap,
        );
    }

    commands.insert_resource(tex_store);
    commands.insert_resource(tilemaps_store);

    // Process animation registration commands from Lua
    let mut anim_store = AnimationStore::default();
    for cmd in lua_runtime.drain_animation_commands() {
        process_animation_command(&mut anim_store, cmd);
    }
    commands.insert_resource(anim_store);

    // Change GameState to Playing
    next_state.set(GameStates::Playing);
    info!("Game setup() done, next state set to Playing");
}

pub use crate::systems::gamestate::quit_game;

// Create initial state of the game and observers
pub fn enter_play(
    mut commands: Commands,
    mut worldsignals: ResMut<WorldSignals>,
    mut tracked_groups: ResMut<TrackedGroups>,
    systems_store: Res<SystemsStore>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    // Call Lua on_enter_play function if it exists
    if lua_runtime.has_function("on_enter_play") {
        match lua_runtime.call_function::<_, String>("on_enter_play", ()) {
            Ok(result) => {
                info!("Lua on_enter_play returned: {}", result);
            }
            Err(e) => {
                error!("Error calling on_enter_play: {}", e);
            }
        }
    }

    // Process signal commands queued by Lua (initializes world signals)
    for cmd in lua_runtime.drain_signal_commands() {
        process_signal_command(&mut worldsignals, cmd);
    }

    // Process group commands from Lua (configures which groups to track globally)
    for cmd in lua_runtime.drain_group_commands() {
        process_group_command(&mut tracked_groups, cmd);
    }

    // Update the tracked groups cache for Lua
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);

    // NOTE: World signals (score, high_score, lives, level, scene) are now initialized by Lua in on_enter_play()

    // Finally, run the switch_scene system to spawn initial scene entities
    commands.run_system(
        *systems_store
            .get("switch_scene")
            .expect("switch_scene system not found"),
    );
}

/// Drains and processes the 11 command queues that are common to both [`update`] and
/// [`switch_scene`]. Both contexts queue the same command types after their Lua callback
/// returns; this helper eliminates the duplicated drain loops.
///
/// `switch_scene` additionally drains group and tilemap commands after this call.
fn drain_common_commands(
    lua_runtime: &LuaRuntime,
    commands: &mut Commands,
    entities: &mut EntityProcessing,
    scene_state: &mut GameSceneState,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    bindings: &mut InputBindings,
) {
    for cmd in lua_runtime.drain_signal_commands() {
        process_signal_command(&mut scene_state.world_signals, cmd);
    }
    process_entity_commands(
        commands,
        lua_runtime.drain_entity_commands(),
        &mut entities.cmd_queries,
        &scene_state.systems_store,
        &scene_state.anim_store,
    );
    for cmd in lua_runtime.drain_spawn_commands() {
        process_spawn_command(commands, cmd, &mut scene_state.world_signals);
    }
    for cmd in lua_runtime.drain_clone_commands() {
        process_clone_command(commands, cmd, &mut scene_state.world_signals);
    }
    for cmd in lua_runtime.drain_phase_commands() {
        process_phase_command(&mut entities.luaphase, cmd);
    }
    for cmd in lua_runtime.drain_audio_commands() {
        process_audio_command(audio_cmd_writer, cmd);
    }
    for cmd in lua_runtime.drain_camera_commands() {
        process_camera_command(commands, cmd);
    }
    for cmd in lua_runtime.drain_render_commands() {
        process_render_command(cmd, &mut scene_state.post_process);
    }
    for cmd in lua_runtime.drain_gameconfig_commands() {
        process_gameconfig_command(cmd, &mut scene_state.config);
    }
    for cmd in lua_runtime.drain_camera_follow_commands() {
        process_camera_follow_command(cmd, &mut scene_state.camera_follow);
    }
    for cmd in lua_runtime.drain_input_commands() {
        process_input_command(cmd, bindings);
    }
}

/// Per-frame update system for scene-specific logic.
///
/// This system delegates scene behavior to Lua callbacks:
/// - Calls `on_update_<scene>` callback in Lua for the current scene
/// - Lua can queue signal commands (set_flag, set_string, etc.)
/// - Processes signal commands from Lua
/// - Reacts to flags set by Lua: "switch_scene", "quit_game"
#[allow(clippy::too_many_arguments)]
pub fn update(
    time: Res<WorldTime>,
    input: Res<InputState>,
    mut commands: Commands,
    mut next_game_state: ResMut<NextGameState>,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
) {
    let lua_runtime = &scripting.lua_runtime;
    let delta_sec = time.delta;

    let scene = scene_state
        .world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or("menu".to_string());

    // Update signal cache for Lua to read current values
    lua_runtime.update_signal_cache(scene_state.world_signals.snapshot());
    lua_runtime.update_gameconfig_cache(&scene_state.config);
    if bindings.take_dirty() {
        lua_runtime.update_bindings_cache(&bindings);
    }

    // Create input snapshot and Lua table for callbacks
    let input_snapshot = InputSnapshot::from_input_state(&input);
    let input_table = match lua_runtime.update_input_table(&input_snapshot) {
        Ok(table) => table,
        Err(e) => {
            error!("Error creating input table: {}", e);
            return;
        }
    };

    // Call scene-specific update callback with (input, dt)
    let callback_name = format!("on_update_{}", scene);
    match lua_runtime.get_function(&callback_name) {
        Ok(Some(func)) => {
            if let Err(e) = func.call::<()>((input_table, delta_sec)) {
                error!("Error calling {}: {}", callback_name, e);
            }
        }
        Ok(None) => {}
        Err(e) => {
            error!("Error resolving {}: {}", callback_name, e);
        }
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
    );

    // Check for quit flag (set by Lua)
    if scene_state.world_signals.has_flag("quit_game") {
        scene_state.world_signals.clear_flag("quit_game");
        next_game_state.set(GameStates::Quitting);
        return;
    }

    // Check for scene switch flag (set by Lua)
    if scene_state.world_signals.has_flag("switch_scene") {
        info!("Scene switch requested in world signals.");
        scene_state.world_signals.clear_flag("switch_scene");
        let switch_scene_system = *scene_state
            .systems_store
            .get("switch_scene")
            .expect("switch_scene system not found");
        commands.run_system(switch_scene_system);
    }
}

pub use crate::systems::gamestate::clean_all_entities;
/// Processes scene switching: despawns old entities, calls Lua callbacks,
/// and processes all queued commands for the new scene.
#[allow(clippy::too_many_arguments)]
pub fn switch_scene(
    mut commands: Commands,
    mut scripting: ScriptingContext,
    mut scene_state: GameSceneState,
    tilemaps_store: Res<TilemapStore>,
    tex_store: Res<TextureStore>,
    entities_to_clean: Query<Entity, Without<Persistent>>,
    persistent_entities: Query<Entity, With<Persistent>>,
    mut tracked_groups: ResMut<TrackedGroups>,
    mut entities: EntityProcessing,
    mut bindings: ResMut<InputBindings>,
) {
    let lua_runtime = &scripting.lua_runtime;
    info!("switch_scene: System called!");

    // Clear all command queues FIRST to discard any stale commands from the previous scene
    // that might reference entities about to be despawned. This prevents panics when
    // entity commands are applied after their target entities have been despawned.
    lua_runtime.clear_all_commands();

    scripting.audio_cmd_writer.write(AudioCmd::StopAllMusic);
    for entity in entities_to_clean.iter() {
        commands.entity(entity).despawn();
    }

    // Clear entity registrations for despawned (non-persistent) entities
    let persistent_set: FxHashSet<Entity> = persistent_entities.iter().collect();
    scene_state
        .world_signals
        .clear_non_persistent_entities(&persistent_set);

    // NOTE: tilemaps_store is NOT cleared - tilemaps are assets loaded during setup

    tracked_groups.clear();
    scene_state.world_signals.clear_group_counts();

    let scene = scene_state
        .world_signals
        .get_string("scene")
        .cloned()
        .unwrap_or_else(|| "menu".to_string());

    // Call Lua on_switch_scene function if it exists
    if lua_runtime.has_function("on_switch_scene")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_switch_scene", scene.clone())
    {
        error!("Error calling on_switch_scene: {}", e);
    }

    drain_common_commands(
        lua_runtime,
        &mut commands,
        &mut entities,
        &mut scene_state,
        &mut scripting.audio_cmd_writer,
        &mut bindings,
    );

    // Group tracking and tilemap spawning are scene-switch-only operations.
    for cmd in lua_runtime.drain_group_commands() {
        process_group_command(&mut tracked_groups, cmd);
    }
    lua_runtime.update_tracked_groups_cache(&tracked_groups.groups);
    for cmd in lua_runtime.drain_tilemap_commands() {
        process_tilemap_command(&mut commands, cmd, &tex_store, &tilemaps_store);
    }

    // Refresh the config cache after the drain may have applied GameConfigCmds.
    lua_runtime.update_gameconfig_cache(&scene_state.config);
}
