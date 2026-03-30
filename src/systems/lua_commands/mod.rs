//! Shared command processing utilities for Lua-Rust communication.
//!
//! This module provides unified command processors used by various Lua callback
//! contexts (scene setup, phase callbacks, timer callbacks, collision callbacks, etc.).
//!
//! # Sub-modules
//!
//! - [`entity_cmd`] – [`process_entity_commands`]: runtime entity manipulation
//! - [`spawn_cmd`] – [`process_spawn_command`], [`process_clone_command`]: entity creation
//! - [`parse`] – animation condition conversion helpers
//!
//! # SystemParam bundles
//!
//! - [`EntityCmdQueries`] – mutable queries needed by `process_entity_commands`
//! - [`ContextQueries`] – read-only queries for building entity context tables

mod context;
mod entity_cmd;
mod parse;
mod spawn_cmd;

pub(crate) use context::build_entity_context;
pub use entity_cmd::process_entity_commands;
pub use spawn_cmd::{process_clone_command, process_spawn_command};

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::{Camera2D, Color, Vector2};

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::luatimer::LuaTimer;
use crate::components::mapposition::MapPosition;
use crate::components::phase::Phase;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;

use crate::events::audio::AudioCmd;
use crate::resources::animationstore::AnimationResource;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::{CameraFollowConfig, EasingCurve, FollowMode};
use crate::resources::fontstore::FontStore;
use crate::resources::group::TrackedGroups;
use crate::resources::input_bindings::{InputBindings, binding_from_str};
use crate::resources::lua_runtime::{
    AnimationCmd, AssetCmd, AudioLuaCmd, CameraCmd, CameraFollowCmd, GameConfigCmd, GroupCmd,
    InputCmd, PhaseCmd, RenderCmd, SignalCmd, TilemapCmd,
};
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::tilemapstore::{Tilemap, TilemapStore};
use crate::resources::worldsignals::WorldSignals;
use crate::systems::phase_core::queue_phase_transition;
use log::{error, info, warn};
use std::sync::Arc;

use crate::components::tween::{Easing, LoopMode, Tween, TweenValue};
use crate::resources::lua_runtime::TweenConfig;

/// Build a configured `Tween<T>` from component values and shared config.
pub(crate) fn build_tween<T: TweenValue>(from: T, to: T, config: &TweenConfig) -> Tween<T> {
    let easing = config.easing.parse::<Easing>().unwrap();
    let loop_mode = config.loop_mode.parse::<LoopMode>().unwrap();
    let mut tween = Tween::new(from, to, config.duration)
        .with_easing(easing)
        .with_loop_mode(loop_mode);
    if config.backwards {
        tween = tween.with_backwards();
    }
    tween
}

// ── SystemParam bundles ───────────────────────────────────────────────────────

/// Mutable queries required by [`process_entity_commands`].
///
/// Embed this in any system or SystemParam that needs to call
/// `process_entity_commands`, and pass `&mut entity_cmd_queries` directly.
#[derive(SystemParam)]
pub struct EntityCmdQueries<'w, 's> {
    pub stuckto: Query<'w, 's, &'static StuckTo>,
    pub signals: Query<'w, 's, &'static mut Signals>,
    pub animation: Query<'w, 's, &'static mut Animation>,
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    pub screen_positions: Query<'w, 's, &'static mut ScreenPosition>,
    pub sprites: Query<'w, 's, &'static mut Sprite>,
    pub shaders: Query<'w, 's, &'static mut EntityShader>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
}

/// Bundled read-only queries for building entity context tables.
///
/// This SystemParam includes read-only components that can be shared by systems
/// that also hold mutable command-processing queries.
#[derive(SystemParam)]
pub struct ContextQueries<'w, 's> {
    pub groups: Query<'w, 's, &'static crate::components::group::Group>,
    pub rotations: Query<'w, 's, &'static Rotation>,
    pub scales: Query<'w, 's, &'static Scale>,
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    pub lua_timers: Query<'w, 's, &'static LuaTimer>,
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    pub child_of: Query<'w, 's, &'static ChildOf>,
}

// ── Small per-command-type processors ────────────────────────────────────────
// Each function handles exactly one command enum and is called from the drain
// loops in lua_plugin.rs, luaphase.rs, luatimer.rs, and lua_collision.rs.

/// Process a single audio command from Lua and write to the audio command channel.
pub fn process_audio_command(audio_cmd_writer: &mut MessageWriter<AudioCmd>, cmd: AudioLuaCmd) {
    match cmd {
        AudioLuaCmd::PlayMusic { id, looped } => {
            audio_cmd_writer.write(AudioCmd::PlayMusic { id, looped });
        }
        AudioLuaCmd::PlaySound { id } => {
            audio_cmd_writer.write(AudioCmd::PlayFx { id });
        }
        AudioLuaCmd::PlaySoundPitched { id, pitch } => {
            audio_cmd_writer.write(AudioCmd::PlayFxPitched { id, pitch });
        }
        AudioLuaCmd::StopAllMusic => {
            audio_cmd_writer.write(AudioCmd::StopAllMusic);
        }
        AudioLuaCmd::StopAllSounds => {
            audio_cmd_writer.write(AudioCmd::StopAllFx);
        }
    }
}

/// Process a single signal command from Lua and update world signals.
pub fn process_signal_command(world_signals: &mut WorldSignals, cmd: SignalCmd) {
    match cmd {
        SignalCmd::SetScalar { key, value } => {
            world_signals.set_scalar(&key, value);
        }
        SignalCmd::SetInteger { key, value } => {
            world_signals.set_integer(&key, value);
        }
        SignalCmd::SetFlag { key } => {
            world_signals.set_flag(&key);
        }
        SignalCmd::ClearFlag { key } => {
            world_signals.clear_flag(&key);
        }
        SignalCmd::SetString { key, value } => {
            world_signals.set_string(&key, &value);
        }
        SignalCmd::ClearScalar { key } => {
            world_signals.clear_scalar(&key);
        }
        SignalCmd::ClearInteger { key } => {
            world_signals.clear_integer(&key);
        }
        SignalCmd::ClearString { key } => {
            world_signals.remove_string(&key);
        }
        SignalCmd::SetEntity { key, entity_id } => {
            if let Some(entity) = Entity::try_from_bits(entity_id) {
                world_signals.set_entity(&key, entity);
            }
        }
        SignalCmd::RemoveEntity { key } => {
            world_signals.remove_entity(&key);
        }
    }
}

/// Process a single group command from Lua and update the tracked groups.
pub fn process_group_command(tracked_groups: &mut TrackedGroups, cmd: GroupCmd) {
    match cmd {
        GroupCmd::TrackGroup { name } => {
            tracked_groups.add_group(&name);
        }
        GroupCmd::UntrackGroup { name } => {
            tracked_groups.remove_group(&name);
        }
        GroupCmd::ClearTrackedGroups => {
            tracked_groups.clear();
        }
    }
}

/// Process a single tilemap command from Lua and spawn tiles.
pub fn process_tilemap_command<F>(
    commands: &mut Commands,
    cmd: TilemapCmd,
    tex_store: &TextureStore,
    tilemaps_store: &TilemapStore,
    spawn_tiles_fn: F,
) where
    F: FnOnce(&mut Commands, String, i32, &Tilemap),
{
    match cmd {
        TilemapCmd::SpawnTiles { id } => {
            if let Some(tilemap_info) = tilemaps_store.get(&id) {
                if let Some(tilemap_tex) = tex_store.get(&id) {
                    let tiles_width = tilemap_tex.width;
                    spawn_tiles_fn(commands, id.clone(), tiles_width, tilemap_info);
                    info!("Spawned tiles for tilemap '{}'", id);
                } else {
                    error!("Tilemap texture '{}' not found", id);
                }
            } else {
                error!("Tilemap '{}' not found in store", id);
            }
        }
    }
}

/// Process a single camera command from Lua and update the camera resource.
pub fn process_camera_command(commands: &mut Commands, cmd: CameraCmd) {
    match cmd {
        CameraCmd::SetCamera2D {
            target_x,
            target_y,
            offset_x,
            offset_y,
            rotation,
            zoom,
        } => {
            commands.insert_resource(Camera2DRes(Camera2D {
                target: Vector2 {
                    x: target_x,
                    y: target_y,
                },
                offset: Vector2 {
                    x: offset_x,
                    y: offset_y,
                },
                rotation,
                zoom,
            }));
        }
    }
}

/// Process a single phase command from Lua and apply it to the appropriate entity.
pub fn process_phase_command<C>(phase_query: &mut Query<(Entity, &mut Phase<C>)>, cmd: PhaseCmd)
where
    C: Send + Sync + 'static,
{
    match cmd {
        PhaseCmd::TransitionTo { entity_id, phase } => {
            let entity = Entity::from_bits(entity_id);
            queue_phase_transition(phase_query, entity, phase);
        }
    }
}

/// Process a single asset command from Lua and load the corresponding asset.
///
/// Designed for use during `on_setup` / scene initialization, not hot gameplay paths.
#[allow(clippy::too_many_arguments)]
pub fn process_asset_command<F1, F2>(
    rl: &mut raylib::RaylibHandle,
    th: &raylib::RaylibThread,
    cmd: AssetCmd,
    tex_store: &mut TextureStore,
    tilemaps_store: &mut TilemapStore,
    fonts: &mut FontStore,
    shader_store: &mut ShaderStore,
    audio_cmd_writer: &mut MessageWriter<AudioCmd>,
    load_font_fn: F1,
    load_tilemap_fn: F2,
) where
    F1: FnOnce(
        &mut raylib::RaylibHandle,
        &raylib::RaylibThread,
        &str,
        i32,
    ) -> raylib::prelude::Font,
    F2: FnOnce(
        &mut raylib::RaylibHandle,
        &raylib::RaylibThread,
        &str,
    ) -> (raylib::prelude::Texture2D, Tilemap),
{
    match cmd {
        AssetCmd::Texture { id, path } => match rl.load_texture(th, &path) {
            Ok(tex) => {
                info!("Loaded texture '{}' from '{}'", id, path);
                tex_store.insert(&id, tex);
            }
            Err(e) => {
                error!("Failed to load texture '{}': {}", path, e);
            }
        },
        AssetCmd::Font { id, path, size } => {
            let font = load_font_fn(rl, th, &path, size);
            info!("Loaded font '{}' from '{}'", id, path);
            fonts.add(&id, font);
        }
        AssetCmd::Music { id, path } => {
            info!("Queuing music '{}' from '{}'", id, path);
            audio_cmd_writer.write(AudioCmd::LoadMusic { id, path });
        }
        AssetCmd::Sound { id, path } => {
            info!("Queuing sound '{}' from '{}'", id, path);
            audio_cmd_writer.write(AudioCmd::LoadFx { id, path });
        }
        AssetCmd::Tilemap { id, path } => {
            let (tilemap_tex, tilemap) = load_tilemap_fn(rl, th, &path);
            let tiles_width = tilemap_tex.width;
            info!(
                "Loaded tilemap '{}' from '{}' ({}x{} texture, tile_size={})",
                id, path, tiles_width, tilemap_tex.height, tilemap.tile_size
            );
            tex_store.insert(&id, tilemap_tex);
            tilemaps_store.insert(&id, tilemap);
        }
        AssetCmd::Shader {
            id,
            vs_path,
            fs_path,
        } => {
            let vs_path_c = vs_path.as_deref();
            let fs_path_c = fs_path.as_deref();
            let shader = rl.load_shader(th, vs_path_c, fs_path_c);
            if shader.is_shader_valid() {
                info!(
                    "Loaded shader '{}' (vs: {:?}, fs: {:?})",
                    id, vs_path, fs_path
                );
                shader_store.add(&id, shader);
            } else {
                error!(
                    "Shader '{}' loaded but is invalid (vs: {:?}, fs: {:?})",
                    id, vs_path, fs_path
                );
            }
        }
    }
}

/// Process a single render command from Lua and update post-process state.
pub fn process_render_command(cmd: RenderCmd, post_process: &mut PostProcessShader) {
    match cmd {
        RenderCmd::SetPostProcessShader { ids } => {
            post_process.set_shader_chain(ids.clone());
            match &ids {
                Some(list) if !list.is_empty() => {
                    info!("Post-process shader chain: [{}]", list.join(", "));
                }
                _ => {
                    info!("Post-process shader disabled");
                }
            }
        }
        RenderCmd::SetPostProcessUniform { name, value } => {
            let is_reserved = post_process.set_uniform(&name, value);
            if is_reserved {
                warn!(
                    "'{}' is a reserved uniform name and will be overwritten by the engine",
                    name
                );
            }
        }
        RenderCmd::ClearPostProcessUniform { name } => {
            post_process.clear_uniform(&name);
        }
        RenderCmd::ClearPostProcessUniforms => {
            post_process.clear_uniforms();
        }
    }
}

/// Process a single game config command from Lua.
pub fn process_gameconfig_command(
    cmd: GameConfigCmd,
    config: &mut crate::resources::gameconfig::GameConfig,
) {
    match cmd {
        GameConfigCmd::Fullscreen { enabled } => {
            config.fullscreen = enabled;
        }
        GameConfigCmd::Vsync { enabled } => {
            config.vsync = enabled;
        }
        GameConfigCmd::TargetFps { fps } => {
            config.target_fps = fps;
        }
        GameConfigCmd::RenderSize { width, height } => {
            config.render_width = width;
            config.render_height = height;
        }
        GameConfigCmd::BackgroundColor { r, g, b } => {
            config.background_color = Color::new(r, g, b, 255);
        }
    }
}

/// Process a single camera follow command from Lua.
pub fn process_camera_follow_command(cmd: CameraFollowCmd, config: &mut CameraFollowConfig) {
    match cmd {
        CameraFollowCmd::Enable { enabled } => {
            config.enabled = enabled;
        }
        CameraFollowCmd::SetMode { mode } => match mode.as_str() {
            "instant" => config.mode = FollowMode::Instant,
            "lerp" => config.mode = FollowMode::Lerp,
            "smooth_damp" => config.mode = FollowMode::SmoothDamp,
            other => {
                warn!(
                    "Unknown camera follow mode '{}'; expected \"instant\", \"lerp\", or \"smooth_damp\"",
                    other
                );
            }
        },
        CameraFollowCmd::SetDeadzone { half_w, half_h } => {
            config.mode = FollowMode::Deadzone { half_w, half_h };
        }
        CameraFollowCmd::SetEasing { easing } => match easing.as_str() {
            "linear" => config.easing = EasingCurve::Linear,
            "ease_out" => config.easing = EasingCurve::EaseOut,
            "ease_in" => config.easing = EasingCurve::EaseIn,
            "ease_in_out" => config.easing = EasingCurve::EaseInOut,
            other => {
                warn!(
                    "Unknown camera follow easing '{}'; expected \"linear\", \"ease_out\", \"ease_in\", or \"ease_in_out\"",
                    other
                );
            }
        },
        CameraFollowCmd::SetSpeed { speed } => {
            config.lerp_speed = speed;
        }
        CameraFollowCmd::SetSpring { stiffness, damping } => {
            config.spring_stiffness = stiffness;
            config.spring_damping = damping;
        }
        CameraFollowCmd::SetOffset { x, y } => {
            config.offset = Vector2 { x, y };
        }
        CameraFollowCmd::SetBounds { x, y, w, h } => {
            config.bounds = Some(raylib::prelude::Rectangle {
                x,
                y,
                width: w,
                height: h,
            });
        }
        CameraFollowCmd::ClearBounds => {
            config.bounds = None;
        }
        CameraFollowCmd::ResetVelocity => {
            config.velocity = Vector2 { x: 0.0, y: 0.0 };
        }
    }
}

/// Process a single input rebinding command from Lua.
pub fn process_input_command(cmd: InputCmd, bindings: &mut InputBindings) {
    use crate::resources::lua_runtime::action_from_str;
    match cmd {
        InputCmd::Rebind { action, key } => {
            let Some(a) = action_from_str(&action) else {
                log::warn!("rebind_action: unknown action '{}'", action);
                return;
            };
            let Some(b) = binding_from_str(&key) else {
                log::warn!("rebind_action: unknown binding '{}'", key);
                return;
            };
            bindings.rebind(a, b);
        }
        InputCmd::AddBinding { action, key } => {
            let Some(a) = action_from_str(&action) else {
                log::warn!("add_binding: unknown action '{}'", action);
                return;
            };
            let Some(b) = binding_from_str(&key) else {
                log::warn!("add_binding: unknown binding '{}'", key);
                return;
            };
            bindings.add_binding(a, b);
        }
    }
}

/// Process a single animation registration command from Lua.
pub fn process_animation_command(
    anim_store: &mut rustc_hash::FxHashMap<String, AnimationResource>,
    cmd: AnimationCmd,
) {
    match cmd {
        AnimationCmd::RegisterAnimation {
            id,
            tex_key,
            pos_x,
            pos_y,
            horizontal_displacement,
            vertical_displacement,
            frame_count,
            fps,
            looped,
        } => {
            anim_store.insert(
                id.clone(),
                AnimationResource {
                    tex_key: Arc::from(tex_key),
                    position: Vector2 { x: pos_x, y: pos_y },
                    horizontal_displacement,
                    vertical_displacement,
                    frame_count,
                    fps,
                    looped,
                },
            );
            info!(
                "Registered animation '{}' ({} frames, {} fps)",
                id, frame_count, fps
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::process_audio_command;
    use crate::events::audio::AudioCmd;
    use crate::resources::lua_runtime::AudioLuaCmd;
    use bevy_ecs::message::Messages;
    use bevy_ecs::prelude::{MessageReader, MessageWriter, World};
    use bevy_ecs::system::SystemState;

    #[test]
    fn stop_all_sounds_maps_to_stop_all_fx() {
        let mut world = World::new();
        world.insert_resource(Messages::<AudioCmd>::default());

        let mut system_state = SystemState::<MessageWriter<AudioCmd>>::new(&mut world);
        {
            let mut writer = system_state.get_mut(&mut world);
            process_audio_command(&mut writer, AudioLuaCmd::StopAllSounds);
        }
        system_state.apply(&mut world);

        world.resource_mut::<Messages<AudioCmd>>().update();

        let mut reader_state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
        let mut reader = reader_state.get_mut(&mut world);
        let cmds: Vec<_> = reader.read().collect();

        assert_eq!(cmds.len(), 1);
        assert!(matches!(cmds[0], AudioCmd::StopAllFx));
    }
}
