//! Engine table API registration for the Lua runtime.
//!
//! Registers all `engine.*` functions in the Lua global `engine` table.

use super::commands::*;
use super::entity_builder::LuaEntityBuilder;
use super::runtime::{LuaAppData, LuaRuntime};
use log::{error, info, warn};
use mlua::prelude::*;

/// Pushes function metadata to `engine.__meta.functions[name]`.
fn push_fn_meta(
    lua: &Lua,
    meta_fns: &LuaTable,
    name: &str,
    desc: &str,
    category: &str,
    params: &[(&str, &str)],
    returns: Option<&str>,
) -> LuaResult<()> {
    let tbl = lua.create_table()?;
    tbl.set("description", desc)?;
    tbl.set("category", category)?;
    let params_tbl = lua.create_table()?;
    for (i, (pname, ptype)) in params.iter().enumerate() {
        let p = lua.create_table()?;
        p.set("name", *pname)?;
        p.set("type", *ptype)?;
        params_tbl.set(i + 1, p)?;
    }
    tbl.set("params", params_tbl)?;
    if let Some(ret) = returns {
        let r = lua.create_table()?;
        r.set("type", ret)?;
        tbl.set("returns", r)?;
    }
    meta_fns.set(name, tbl)?;
    Ok(())
}

/// Registers a Lua function that pushes a command to a queue in `LuaAppData`.
macro_rules! register_cmd {
    // Variant with metadata
    ($engine:expr, $lua:expr, $meta_fns:expr, $name:expr, $queue:ident,
     |$args:pat_param| $arg_ty:ty, $cmd:expr,
     desc = $desc:expr, cat = $cat:expr,
     params = [ $( ($pname:expr, $pty:expr) ),* $(,)? ]
     $(, returns = $ret:expr )?
    ) => {
        $engine.set(
            $name,
            $lua.create_function(|lua, $args: $arg_ty| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .$queue
                    .borrow_mut()
                    .push($cmd);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &$lua, &$meta_fns, $name, $desc, $cat,
            &[ $(($pname, $pty)),* ],
            register_cmd!(@opt_ret $($ret)?)
        )?;
    };
    // Helper to produce Option<&str> from optional returns
    (@opt_ret $ret:expr) => { Some($ret) };
    (@opt_ret) => { None };
}

/// Registers a batch of entity commands with a name prefix to a specific queue.
macro_rules! register_entity_cmds {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, [
        $( ($name:literal,
            |$args:pat_param| $arg_ty:ty, $cmd:expr,
            desc = $desc:expr,
            params = [ $( ($pname:expr, $pty:expr) ),* $(,)? ]
        ) ),* $(,)?
    ]) => {
        $(
            register_cmd!($engine, $lua, $meta_fns, concat!($prefix, $name), $queue,
                |$args| $arg_ty, $cmd,
                desc = $desc, cat = "entity",
                params = [ $( ($pname, $pty) ),* ]);
        )*
    };
}

/// Defines and registers all entity commands for a given prefix and queue.
/// Called with `""` prefix for regular commands, `"collision_"` for collision commands.
macro_rules! define_entity_cmds {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident) => {
        register_entity_cmds!($engine, $lua, $meta_fns, $prefix, $queue, [
            ("entity_despawn", |entity_id| u64, EntityCmd::Despawn { entity_id },
                desc = "Despawn an entity",
                params = [("entity_id", "integer")]),
            ("entity_menu_despawn", |entity_id| u64, EntityCmd::MenuDespawn { entity_id },
                desc = "Despawn a menu entity and its children",
                params = [("entity_id", "integer")]),
            ("release_stuckto", |entity_id| u64, EntityCmd::ReleaseStuckTo { entity_id },
                desc = "Release entity from its StuckTo target, restoring stored velocity",
                params = [("entity_id", "integer")]),
            ("entity_signal_set_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalSetFlag { entity_id, flag },
                desc = "Set a flag on an entity's signals",
                params = [("entity_id", "integer"), ("flag", "string")]),
            ("entity_signal_clear_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalClearFlag { entity_id, flag },
                desc = "Clear a flag on an entity's signals",
                params = [("entity_id", "integer"), ("flag", "string")]),
            ("entity_set_velocity",
                |(entity_id, vx, vy)| (u64, f32, f32), EntityCmd::SetVelocity { entity_id, vx, vy },
                desc = "Set entity velocity",
                params = [("entity_id", "integer"), ("vx", "number"), ("vy", "number")]),
            ("entity_insert_stuckto",
                |(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)|
                (u64, u64, bool, bool, f32, f32, f32, f32),
                EntityCmd::InsertStuckTo {
                    entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy,
                },
                desc = "Attach entity to a target entity",
                params = [("entity_id", "integer"), ("target_id", "integer"),
                          ("follow_x", "boolean"), ("follow_y", "boolean"),
                          ("offset_x", "number"), ("offset_y", "number"),
                          ("stored_vx", "number"), ("stored_vy", "number")]),
            ("entity_restart_animation", |entity_id| u64, EntityCmd::RestartAnimation { entity_id },
                desc = "Restart entity animation from frame 0",
                params = [("entity_id", "integer")]),
            ("entity_set_animation",
                |(entity_id, animation_key)| (u64, String), EntityCmd::SetAnimation { entity_id, animation_key },
                desc = "Set entity animation by key",
                params = [("entity_id", "integer"), ("animation_key", "string")]),
            ("entity_set_sprite_flip",
                |(entity_id, flip_h, flip_v)| (u64, bool, bool), EntityCmd::SetSpriteFlip { entity_id, flip_h, flip_v },
                desc = "Set sprite flip on horizontal and vertical axes",
                params = [("entity_id", "integer"), ("flip_h", "boolean"), ("flip_v", "boolean")]),
            ("entity_insert_lua_timer",
                |(entity_id, duration, callback)| (u64, f32, String),
                EntityCmd::InsertLuaTimer { entity_id, duration, callback },
                desc = "Insert a Lua timer on an entity",
                params = [("entity_id", "integer"), ("duration", "number"), ("callback", "string")]),
            ("entity_remove_lua_timer", |entity_id| u64, EntityCmd::RemoveLuaTimer { entity_id },
                desc = "Remove the Lua timer from an entity",
                params = [("entity_id", "integer")]),
            ("entity_insert_ttl",
                |(entity_id, seconds)| (u64, f32), EntityCmd::InsertTtl { entity_id, seconds },
                desc = "Insert a time-to-live component on an entity",
                params = [("entity_id", "integer"), ("seconds", "number")]),
            ("entity_insert_tween_position",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenPosition {
                    entity_id, from_x, from_y, to_x, to_y,
                    config: TweenConfig { duration, easing, loop_mode, backwards },
                },
                desc = "Insert a position tween on an entity",
                params = [("entity_id", "integer"), ("from_x", "number"), ("from_y", "number"),
                          ("to_x", "number"), ("to_y", "number"), ("duration", "number"),
                          ("easing", "string"), ("loop_mode", "string"), ("backwards", "boolean")]),
            ("entity_insert_tween_rotation",
                |(entity_id, from, to, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenRotation {
                    entity_id, from, to,
                    config: TweenConfig { duration, easing, loop_mode, backwards },
                },
                desc = "Insert a rotation tween on an entity",
                params = [("entity_id", "integer"), ("from", "number"), ("to", "number"),
                          ("duration", "number"), ("easing", "string"), ("loop_mode", "string"),
                          ("backwards", "boolean")]),
            ("entity_insert_tween_scale",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenScale {
                    entity_id, from_x, from_y, to_x, to_y,
                    config: TweenConfig { duration, easing, loop_mode, backwards },
                },
                desc = "Insert a scale tween on an entity",
                params = [("entity_id", "integer"), ("from_x", "number"), ("from_y", "number"),
                          ("to_x", "number"), ("to_y", "number"), ("duration", "number"),
                          ("easing", "string"), ("loop_mode", "string"), ("backwards", "boolean")]),
            ("entity_remove_tween_position", |entity_id| u64, EntityCmd::RemoveTweenPosition { entity_id },
                desc = "Remove position tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_remove_tween_rotation", |entity_id| u64, EntityCmd::RemoveTweenRotation { entity_id },
                desc = "Remove rotation tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_remove_tween_scale", |entity_id| u64, EntityCmd::RemoveTweenScale { entity_id },
                desc = "Remove scale tween from an entity",
                params = [("entity_id", "integer")]),
            ("entity_set_rotation",
                |(entity_id, degrees)| (u64, f32), EntityCmd::SetRotation { entity_id, degrees },
                desc = "Set entity rotation in degrees",
                params = [("entity_id", "integer"), ("degrees", "number")]),
            ("entity_set_scale",
                |(entity_id, sx, sy)| (u64, f32, f32), EntityCmd::SetScale { entity_id, sx, sy },
                desc = "Set entity scale",
                params = [("entity_id", "integer"), ("sx", "number"), ("sy", "number")]),
            ("entity_signal_set_scalar",
                |(entity_id, key, value)| (u64, String, f32),
                EntityCmd::SignalSetScalar { entity_id, key, value },
                desc = "Set a scalar signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "number")]),
            ("entity_signal_set_string",
                |(entity_id, key, value)| (u64, String, String),
                EntityCmd::SignalSetString { entity_id, key, value },
                desc = "Set a string signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "string")]),
            ("entity_add_force",
                |(entity_id, name, x, y, enabled)| (u64, String, f32, f32, bool),
                EntityCmd::AddForce { entity_id, name, x, y, enabled },
                desc = "Add a named acceleration force to an entity",
                params = [("entity_id", "integer"), ("name", "string"),
                          ("x", "number"), ("y", "number"), ("enabled", "boolean")]),
            ("entity_remove_force",
                |(entity_id, name)| (u64, String), EntityCmd::RemoveForce { entity_id, name },
                desc = "Remove a named force from an entity",
                params = [("entity_id", "integer"), ("name", "string")]),
            ("entity_set_force_enabled",
                |(entity_id, name, enabled)| (u64, String, bool),
                EntityCmd::SetForceEnabled { entity_id, name, enabled },
                desc = "Enable or disable a named force on an entity",
                params = [("entity_id", "integer"), ("name", "string"), ("enabled", "boolean")]),
            ("entity_set_force_value",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::SetForceValue { entity_id, name, x, y },
                desc = "Set the acceleration value of a named force",
                params = [("entity_id", "integer"), ("name", "string"), ("x", "number"), ("y", "number")]),
            ("entity_set_friction",
                |(entity_id, friction)| (u64, f32), EntityCmd::SetFriction { entity_id, friction },
                desc = "Set entity friction",
                params = [("entity_id", "integer"), ("friction", "number")]),
            ("entity_set_max_speed",
                |(entity_id, max_speed)| (u64, Option<f32>), EntityCmd::SetMaxSpeed { entity_id, max_speed },
                desc = "Set entity max speed (nil to remove)",
                params = [("entity_id", "integer"), ("max_speed", "number?")]),
            ("entity_freeze", |entity_id| u64, EntityCmd::FreezeEntity { entity_id },
                desc = "Freeze entity (zero velocity, ignore forces)",
                params = [("entity_id", "integer")]),
            ("entity_unfreeze", |entity_id| u64, EntityCmd::UnfreezeEntity { entity_id },
                desc = "Unfreeze entity",
                params = [("entity_id", "integer")]),
            ("entity_set_speed",
                |(entity_id, speed)| (u64, f32), EntityCmd::SetSpeed { entity_id, speed },
                desc = "Set entity speed (scales velocity to this magnitude)",
                params = [("entity_id", "integer"), ("speed", "number")]),
            ("entity_set_position",
                |(entity_id, x, y)| (u64, f32, f32), EntityCmd::SetPosition { entity_id, x, y },
                desc = "Set entity world position",
                params = [("entity_id", "integer"), ("x", "number"), ("y", "number")]),
            ("entity_set_screen_position",
                |(entity_id, x, y)| (u64, f32, f32), EntityCmd::SetScreenPosition { entity_id, x, y },
                desc = "Set entity screen-space position",
                params = [("entity_id", "integer"), ("x", "number"), ("y", "number")]),
            ("entity_signal_set_integer",
                |(entity_id, key, value)| (u64, String, i32),
                EntityCmd::SignalSetInteger { entity_id, key, value },
                desc = "Set an integer signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "integer")]),
            ("entity_set_shader",
                |(entity_id, key)| (u64, String), EntityCmd::SetShader { entity_id, key },
                desc = "Set per-entity shader by key",
                params = [("entity_id", "integer"), ("key", "string")]),
            ("entity_remove_shader", |entity_id| u64, EntityCmd::RemoveShader { entity_id },
                desc = "Remove per-entity shader",
                params = [("entity_id", "integer")]),
            ("entity_set_tint",
                |(entity_id, r, g, b, a)| (u64, u8, u8, u8, u8),
                EntityCmd::SetTint { entity_id, r, g, b, a },
                desc = "Set entity tint color (RGBA 0-255)",
                params = [("entity_id", "integer"), ("r", "integer"), ("g", "integer"),
                          ("b", "integer"), ("a", "integer")]),
            ("entity_remove_tint", |entity_id| u64, EntityCmd::RemoveTint { entity_id },
                desc = "Remove entity tint",
                params = [("entity_id", "integer")]),
            ("entity_shader_set_float",
                |(entity_id, name, value)| (u64, String, f32),
                EntityCmd::ShaderSetFloat { entity_id, name, value },
                desc = "Set a float uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("value", "number")]),
            ("entity_shader_set_int",
                |(entity_id, name, value)| (u64, String, i32),
                EntityCmd::ShaderSetInt { entity_id, name, value },
                desc = "Set an int uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("value", "integer")]),
            ("entity_shader_set_vec2",
                |(entity_id, name, x, y)| (u64, String, f32, f32),
                EntityCmd::ShaderSetVec2 { entity_id, name, x, y },
                desc = "Set a vec2 uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"), ("x", "number"), ("y", "number")]),
            ("entity_shader_set_vec4",
                |(entity_id, name, x, y, z, w)| (u64, String, f32, f32, f32, f32),
                EntityCmd::ShaderSetVec4 { entity_id, name, x, y, z, w },
                desc = "Set a vec4 uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string"),
                          ("x", "number"), ("y", "number"), ("z", "number"), ("w", "number")]),
            ("entity_shader_clear_uniform",
                |(entity_id, name)| (u64, String), EntityCmd::ShaderClearUniform { entity_id, name },
                desc = "Clear a uniform on entity shader",
                params = [("entity_id", "integer"), ("name", "string")]),
            ("entity_shader_clear_uniforms", |entity_id| u64, EntityCmd::ShaderClearUniforms { entity_id },
                desc = "Clear all uniforms on entity shader",
                params = [("entity_id", "integer")]),
            ("entity_set_parent",
                |(entity_id, parent_id)| (u64, u64),
                EntityCmd::SetParent { entity_id, parent_id },
                desc = "Set the parent of an entity for transform hierarchy",
                params = [("entity_id", "integer"), ("parent_id", "integer")]),
            ("entity_remove_parent", |entity_id| u64,
                EntityCmd::RemoveParent { entity_id },
                desc = "Remove entity from its parent, snapping to current world position",
                params = [("entity_id", "integer")]),
            ("entity_set_camera_target",
                |(entity_id, priority)| (u64, u8),
                EntityCmd::SetCameraTarget { entity_id, priority },
                desc = "Set CameraTarget component on an entity (higher priority wins)",
                params = [("entity_id", "integer"), ("priority", "integer")]),
            ("entity_remove_camera_target", |entity_id| u64,
                EntityCmd::RemoveCameraTarget { entity_id },
                desc = "Remove CameraTarget component from an entity",
                params = [("entity_id", "integer")]),
        ]);
    };
}

impl LuaRuntime {
    /// Registers the base `engine` table with logging functions.
    pub(super) fn register_base_api(&self) -> LuaResult<()> {
        let engine = self.lua.create_table()?;

        // Create __meta table with functions and classes subtables
        let meta = self.lua.create_table()?;
        let meta_fns = self.lua.create_table()?;
        let meta_classes = self.lua.create_table()?;
        let meta_types = self.lua.create_table()?;
        let meta_enums = self.lua.create_table()?;
        let meta_callbacks = self.lua.create_table()?;
        meta.set("functions", &meta_fns)?;
        meta.set("classes", &meta_classes)?;
        meta.set("types", &meta_types)?;
        meta.set("enums", &meta_enums)?;
        meta.set("callbacks", &meta_callbacks)?;
        engine.set("__meta", meta)?;

        engine.set(
            "log",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log",
            "General purpose logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        engine.set(
            "log_info",
            self.lua.create_function(|_, msg: String| {
                info!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_info",
            "Info level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        engine.set(
            "log_warn",
            self.lua.create_function(|_, msg: String| {
                warn!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_warn",
            "Warning level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        engine.set(
            "log_error",
            self.lua.create_function(|_, msg: String| {
                error!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "log_error",
            "Error level logging",
            "base",
            &[("message", "string")],
            None,
        )?;

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }

    pub(super) fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_texture",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Texture { id, path },
            desc = "Load a texture from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_font",
            asset_commands,
            |(id, path, size)| (String, String, i32),
            AssetCmd::Font { id, path, size },
            desc = "Load a font from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string"), ("size", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_music",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Music { id, path },
            desc = "Load music from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_sound",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Sound { id, path },
            desc = "Load a sound effect from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "load_tilemap",
            asset_commands,
            |(id, path)| (String, String),
            AssetCmd::Tilemap { id, path },
            desc = "Load a tilemap from file",
            cat = "asset",
            params = [("id", "string"), ("path", "string")]
        );
        Ok(())
    }

    pub(super) fn register_spawn_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new()))?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "spawn",
            "Create a new entity builder",
            "spawn",
            &[],
            Some("EntityBuilder"),
        )?;

        engine.set(
            "clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_clone(source_key))
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "clone",
            "Clone a registered entity with optional overrides",
            "spawn",
            &[("source_key", "string")],
            Some("EntityBuilder"),
        )?;

        Ok(())
    }

    pub(super) fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_music",
            audio_commands,
            |(id, looped)| (String, bool),
            AudioLuaCmd::PlayMusic { id, looped },
            desc = "Play music track",
            cat = "audio",
            params = [("id", "string"), ("looped", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_sound",
            audio_commands,
            |id| String,
            AudioLuaCmd::PlaySound { id },
            desc = "Play a sound effect",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "play_sound_pitched",
            audio_commands,
            |(id, pitch)| (String, f32),
            AudioLuaCmd::PlaySoundPitched { id, pitch },
            desc = "Play a sound effect with pitch override (1.0 = normal)",
            cat = "audio",
            params = [("id", "string"), ("pitch", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_music",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllMusic,
            desc = "Stop all playing music",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_all_sounds",
            audio_commands,
            |()| (),
            AudioLuaCmd::StopAllSounds,
            desc = "Stop all playing sounds",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "stop_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::StopMusic { id },
            desc = "Stop a specific music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "pause_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::PauseMusic { id },
            desc = "Pause a specific music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "resume_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::ResumeMusic { id },
            desc = "Resume a previously paused music track",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_music_volume",
            audio_commands,
            |(id, vol)| (String, f32),
            AudioLuaCmd::SetMusicVolume { id, vol },
            desc = "Set the volume of a music track (0.0 to 1.0)",
            cat = "audio",
            params = [("id", "string"), ("vol", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_music",
            audio_commands,
            |id| String,
            AudioLuaCmd::UnloadMusic { id },
            desc = "Unload a specific music track from memory",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_all_music",
            audio_commands,
            |()| (),
            AudioLuaCmd::UnloadAllMusic,
            desc = "Unload all music tracks from memory",
            cat = "audio",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_sound",
            audio_commands,
            |id| String,
            AudioLuaCmd::UnloadSound { id },
            desc = "Unload a specific sound effect from memory",
            cat = "audio",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "unload_all_sounds",
            audio_commands,
            |()| (),
            AudioLuaCmd::UnloadAllSounds,
            desc = "Unload all sound effects from memory",
            cat = "audio",
            params = []
        );
        Ok(())
    }

    pub(super) fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "get_scalar",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().scalars.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_scalar",
            "Get a world signal scalar value",
            "signal",
            &[("key", "string")],
            Some("number?"),
        )?;

        engine.set(
            "get_integer",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().integers.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_integer",
            "Get a world signal integer value",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        engine.set(
            "get_string",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().strings.get(&key).cloned());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_string",
            "Get a world signal string value",
            "signal",
            &[("key", "string")],
            Some("string?"),
        )?;

        engine.set(
            "has_flag",
            self.lua.create_function(|lua, key: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_snapshot.borrow().flags.contains(&key))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "has_flag",
            "Check if a world signal flag is set",
            "signal",
            &[("key", "string")],
            Some("boolean"),
        )?;

        engine.set(
            "get_group_count",
            self.lua.create_function(|lua, group: String| {
                let count = lua.app_data_ref::<LuaAppData>().and_then(|data| {
                    data.signal_snapshot
                        .borrow()
                        .group_counts
                        .get(&group)
                        .copied()
                });
                Ok(count)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_group_count",
            "Get the count of entities in a tracked group",
            "signal",
            &[("group", "string")],
            Some("integer?"),
        )?;

        engine.set(
            "get_entity",
            self.lua.create_function(|lua, key: String| {
                let entity_id = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().entities.get(&key).copied());
                Ok(entity_id)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_entity",
            "Get a registered entity ID by key",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_scalar",
            signal_commands,
            |(key, value)| (String, f32),
            SignalCmd::SetScalar { key, value },
            desc = "Set a world signal scalar value",
            cat = "signal",
            params = [("key", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_integer",
            signal_commands,
            |(key, value)| (String, i32),
            SignalCmd::SetInteger { key, value },
            desc = "Set a world signal integer value",
            cat = "signal",
            params = [("key", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_string",
            signal_commands,
            |(key, value)| (String, String),
            SignalCmd::SetString { key, value },
            desc = "Set a world signal string value",
            cat = "signal",
            params = [("key", "string"), ("value", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_flag",
            signal_commands,
            |key| String,
            SignalCmd::SetFlag { key },
            desc = "Set a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );

        engine.set(
            "change_scene",
            self.lua.create_function(|lua, scene_name: String| {
                let data = lua
                    .app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                let mut cmds = data.signal_commands.borrow_mut();
                cmds.push(SignalCmd::SetString {
                    key: "scene".into(),
                    value: scene_name,
                });
                cmds.push(SignalCmd::SetFlag {
                    key: "switch_scene".into(),
                });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "change_scene",
            "Switch to a new scene by name (sets scene string + switch_scene flag)",
            "base",
            &[("scene_name", "string")],
            None,
        )?;

        engine.set(
            "quit",
            self.lua.create_function(|lua, ()| {
                let data = lua
                    .app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                data.signal_commands.borrow_mut().push(SignalCmd::SetFlag {
                    key: "quit_game".into(),
                });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "quit",
            "Quit the game engine (sets quit_game flag)",
            "base",
            &[],
            None,
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_flag",
            signal_commands,
            |key| String,
            SignalCmd::ClearFlag { key },
            desc = "Clear a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_scalar",
            signal_commands,
            |key| String,
            SignalCmd::ClearScalar { key },
            desc = "Clear a world signal scalar",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_integer",
            signal_commands,
            |key| String,
            SignalCmd::ClearInteger { key },
            desc = "Clear a world signal integer",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_string",
            signal_commands,
            |key| String,
            SignalCmd::ClearString { key },
            desc = "Clear a world signal string",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_entity",
            signal_commands,
            |(key, entity_id)| (String, u64),
            SignalCmd::SetEntity { key, entity_id },
            desc = "Register an entity ID in world signals",
            cat = "signal",
            params = [("key", "string"), ("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "remove_entity",
            signal_commands,
            |key| String,
            SignalCmd::RemoveEntity { key },
            desc = "Remove a registered entity from world signals",
            cat = "signal",
            params = [("key", "string")]
        );

        Ok(())
    }

    pub(super) fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "phase_transition",
            phase_commands,
            |(entity_id, phase)| (u64, String),
            PhaseCmd::TransitionTo { entity_id, phase },
            desc = "Transition an entity to a new phase",
            cat = "phase",
            params = [("entity_id", "integer"), ("phase", "string")]
        );
        Ok(())
    }

    pub(super) fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
        Ok(())
    }

    pub(super) fn register_group_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "track_group",
            group_commands,
            |name| String,
            GroupCmd::TrackGroup { name },
            desc = "Start tracking a named entity group",
            cat = "group",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "untrack_group",
            group_commands,
            |name| String,
            GroupCmd::UntrackGroup { name },
            desc = "Stop tracking a named entity group",
            cat = "group",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_tracked_groups",
            group_commands,
            |()| (),
            GroupCmd::ClearTrackedGroups,
            desc = "Stop tracking all entity groups",
            cat = "group",
            params = []
        );

        engine.set(
            "has_tracked_group",
            self.lua.create_function(|lua, name: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.tracked_groups.borrow().contains(&name))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "has_tracked_group",
            "Check if a group is being tracked",
            "group",
            &[("name", "string")],
            Some("boolean"),
        )?;

        Ok(())
    }

    pub(super) fn register_tilemap_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "spawn_tiles",
            tilemap_commands,
            |id| String,
            TilemapCmd::SpawnTiles { id },
            desc = "Spawn tilemap entities from a loaded tilemap",
            cat = "tilemap",
            params = [("id", "string")]
        );
        Ok(())
    }

    pub(super) fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_camera",
            camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (
                f32, f32, f32, f32, f32, f32
            ),
            CameraCmd::SetCamera2D {
                target_x,
                target_y,
                offset_x,
                offset_y,
                rotation,
                zoom
            },
            desc = "Set the 2D camera target, offset, rotation and zoom",
            cat = "camera",
            params = [
                ("target_x", "number"),
                ("target_y", "number"),
                ("offset_x", "number"),
                ("offset_y", "number"),
                ("rotation", "number"),
                ("zoom", "number")
            ]
        );
        Ok(())
    }

    pub(super) fn register_camera_follow_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_enable",
            camera_follow_commands,
            |enabled| bool,
            CameraFollowCmd::Enable { enabled },
            desc = "Enable or disable the camera follow system",
            cat = "camera",
            params = [("enabled", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_mode",
            camera_follow_commands,
            |mode| String,
            CameraFollowCmd::SetMode { mode },
            desc = "Set camera follow mode (\"instant\", \"lerp\", \"smooth_damp\")",
            cat = "camera",
            params = [("mode", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_deadzone",
            camera_follow_commands,
            |(half_w, half_h)| (f32, f32),
            CameraFollowCmd::SetDeadzone { half_w, half_h },
            desc = "Set camera follow mode to deadzone with given half-dimensions",
            cat = "camera",
            params = [("half_w", "number"), ("half_h", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_easing",
            camera_follow_commands,
            |easing| String,
            CameraFollowCmd::SetEasing { easing },
            desc = "Set camera follow easing curve (\"linear\", \"ease_out\", \"ease_in\", \"ease_in_out\")",
            cat = "camera",
            params = [("easing", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_speed",
            camera_follow_commands,
            |speed| f32,
            CameraFollowCmd::SetSpeed { speed },
            desc = "Set camera follow lerp speed",
            cat = "camera",
            params = [("speed", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_spring",
            camera_follow_commands,
            |(stiffness, damping)| (f32, f32),
            CameraFollowCmd::SetSpring { stiffness, damping },
            desc = "Set camera follow spring stiffness and damping",
            cat = "camera",
            params = [("stiffness", "number"), ("damping", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_offset",
            camera_follow_commands,
            |(x, y)| (f32, f32),
            CameraFollowCmd::SetOffset { x, y },
            desc = "Set camera follow offset from target position",
            cat = "camera",
            params = [("x", "number"), ("y", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_set_bounds",
            camera_follow_commands,
            |(x, y, w, h)| (f32, f32, f32, f32),
            CameraFollowCmd::SetBounds { x, y, w, h },
            desc = "Set camera follow world-space bounds (x, y, width, height)",
            cat = "camera",
            params = [
                ("x", "number"),
                ("y", "number"),
                ("w", "number"),
                ("h", "number")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_clear_bounds",
            camera_follow_commands,
            |()| (),
            CameraFollowCmd::ClearBounds,
            desc = "Clear camera follow bounds",
            cat = "camera",
            params = []
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "camera_follow_reset_velocity",
            camera_follow_commands,
            |()| (),
            CameraFollowCmd::ResetVelocity,
            desc = "Reset camera follow spring velocity to zero",
            cat = "camera",
            params = []
        );
        Ok(())
    }

    pub(super) fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        define_entity_cmds!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_entity_commands
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_play_sound",
            collision_audio_commands,
            |id| String,
            AudioLuaCmd::PlaySound { id },
            desc = "Play a sound effect (collision context)",
            cat = "collision",
            params = [("id", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_play_sound_pitched",
            collision_audio_commands,
            |(id, pitch)| (String, f32),
            AudioLuaCmd::PlaySoundPitched { id, pitch },
            desc = "Play a sound effect with pitch override (collision context)",
            cat = "collision",
            params = [("id", "string"), ("pitch", "number")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_scalar",
            collision_signal_commands,
            |(key, value)| (String, f32),
            SignalCmd::SetScalar { key, value },
            desc = "Set a world signal scalar (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_integer",
            collision_signal_commands,
            |(key, value)| (String, i32),
            SignalCmd::SetInteger { key, value },
            desc = "Set a world signal integer (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_string",
            collision_signal_commands,
            |(key, value)| (String, String),
            SignalCmd::SetString { key, value },
            desc = "Set a world signal string (collision context)",
            cat = "collision",
            params = [("key", "string"), ("value", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_flag",
            collision_signal_commands,
            |flag| String,
            SignalCmd::SetFlag { key: flag },
            desc = "Set a world signal flag (collision context)",
            cat = "collision",
            params = [("flag", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_flag",
            collision_signal_commands,
            |flag| String,
            SignalCmd::ClearFlag { key: flag },
            desc = "Clear a world signal flag (collision context)",
            cat = "collision",
            params = [("flag", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_scalar",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearScalar { key },
            desc = "Clear a world signal scalar (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_integer",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearInteger { key },
            desc = "Clear a world signal integer (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_clear_string",
            collision_signal_commands,
            |key| String,
            SignalCmd::ClearString { key },
            desc = "Clear a world signal string (collision context)",
            cat = "collision",
            params = [("key", "string")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_phase_transition",
            collision_phase_commands,
            |(entity_id, phase)| (u64, String),
            PhaseCmd::TransitionTo { entity_id, phase },
            desc = "Transition an entity to a new phase (collision context)",
            cat = "collision",
            params = [("entity_id", "integer"), ("phase", "string")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_set_camera",
            collision_camera_commands,
            |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (
                f32, f32, f32, f32, f32, f32
            ),
            CameraCmd::SetCamera2D {
                target_x,
                target_y,
                offset_x,
                offset_y,
                rotation,
                zoom
            },
            desc = "Set the 2D camera (collision context)",
            cat = "collision",
            params = [
                ("target_x", "number"),
                ("target_y", "number"),
                ("offset_x", "number"),
                ("offset_y", "number"),
                ("rotation", "number"),
                ("zoom", "number")
            ]
        );

        engine.set(
            "collision_spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new_collision()))?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "collision_spawn",
            "Create a new entity builder (collision context)",
            "collision",
            &[],
            Some("CollisionEntityBuilder"),
        )?;

        engine.set(
            "collision_clone",
            self.lua.create_function(|_, source_key: String| {
                Ok(LuaEntityBuilder::new_collision_clone(source_key))
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "collision_clone",
            "Clone a registered entity (collision context)",
            "collision",
            &[("source_key", "string")],
            Some("CollisionEntityBuilder"),
        )?;

        Ok(())
    }

    pub(super) fn register_animation_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "register_animation",
            animation_commands,
            |(
                id,
                tex_key,
                pos_x,
                pos_y,
                horizontal_displacement,
                vertical_displacement,
                frame_count,
                fps,
                looped,
            )| (String, String, f32, f32, f32, f32, usize, f32, bool),
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
            },
            desc = "Register an animation definition",
            cat = "animation",
            params = [
                ("id", "string"),
                ("tex_key", "string"),
                ("pos_x", "number"),
                ("pos_y", "number"),
                ("horizontal_displacement", "number"),
                ("vertical_displacement", "number"),
                ("frame_count", "integer"),
                ("fps", "number"),
                ("looped", "boolean")
            ]
        );
        Ok(())
    }

    pub(super) fn register_render_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "load_shader",
            self.lua.create_function(
                |lua, (id, vs_path, fs_path): (String, Option<String>, Option<String>)| {
                    if vs_path.is_none() && fs_path.is_none() {
                        return Err(LuaError::runtime(
                            "load_shader: at least one of vs_path or fs_path must be provided",
                        ));
                    }
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::Shader {
                            id,
                            vs_path,
                            fs_path,
                        });
                    Ok(())
                },
            )?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "load_shader",
            "Load a shader (at least one of vs_path/fs_path required)",
            "render",
            &[
                ("id", "string"),
                ("vs_path", "string?"),
                ("fs_path", "string?"),
            ],
            None,
        )?;

        engine.set(
            "post_process_shader",
            self.lua.create_function(|lua, value: LuaValue| {
                let ids: Option<Vec<String>> = match value {
                    LuaValue::Nil => None,
                    LuaValue::Table(t) => {
                        let mut vec = Vec::new();
                        for pair in t.pairs::<i64, String>() {
                            let (_, id) = pair?;
                            vec.push(id);
                        }
                        if vec.is_empty() {
                            return Err(LuaError::runtime(
                                "post_process_shader: table must contain at least one shader ID",
                            ));
                        }
                        Some(vec)
                    }
                    _ => {
                        return Err(LuaError::runtime(
                            "post_process_shader: expected nil or table of shader IDs",
                        ));
                    }
                };
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .render_commands
                    .borrow_mut()
                    .push(RenderCmd::SetPostProcessShader { ids });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "post_process_shader",
            "Set active post-processing shader chain (nil to clear)",
            "render",
            &[("shader_ids", "string[]?")],
            None,
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_float",
            render_commands,
            |(name, value)| (String, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Float(value)
            },
            desc = "Set a float uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_int",
            render_commands,
            |(name, value)| (String, i32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Int(value)
            },
            desc = "Set an int uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec2",
            render_commands,
            |(name, x, y)| (String, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec2 { x, y }
            },
            desc = "Set a vec2 uniform on post-process shader",
            cat = "render",
            params = [("name", "string"), ("x", "number"), ("y", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_set_vec4",
            render_commands,
            |(name, x, y, z, w)| (String, f32, f32, f32, f32),
            RenderCmd::SetPostProcessUniform {
                name,
                value: UniformValue::Vec4 { x, y, z, w }
            },
            desc = "Set a vec4 uniform on post-process shader",
            cat = "render",
            params = [
                ("name", "string"),
                ("x", "number"),
                ("y", "number"),
                ("z", "number"),
                ("w", "number")
            ]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniform",
            render_commands,
            |name| String,
            RenderCmd::ClearPostProcessUniform { name },
            desc = "Clear a uniform on post-process shader",
            cat = "render",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "post_process_clear_uniforms",
            render_commands,
            |()| (),
            RenderCmd::ClearPostProcessUniforms,
            desc = "Clear all uniforms on post-process shader",
            cat = "render",
            params = []
        );

        Ok(())
    }

    pub(super) fn register_gameconfig_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_fullscreen",
            gameconfig_commands,
            |enabled| bool,
            GameConfigCmd::Fullscreen { enabled },
            desc = "Set fullscreen mode",
            cat = "render",
            params = [("enabled", "boolean")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_vsync",
            gameconfig_commands,
            |enabled| bool,
            GameConfigCmd::Vsync { enabled },
            desc = "Set vertical sync",
            cat = "render",
            params = [("enabled", "boolean")]
        );

        engine.set(
            "set_target_fps",
            self.lua.create_function(|lua, fps: Option<u32>| {
                let fps = fps.unwrap_or(60);
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .gameconfig_commands
                    .borrow_mut()
                    .push(GameConfigCmd::TargetFps { fps });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_target_fps",
            "Set target FPS (nil resets to 60)",
            "render",
            &[("fps", "integer?")],
            None,
        )?;

        engine.set(
            "get_fullscreen",
            self.lua.create_function(|lua, ()| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().fullscreen)
                    .unwrap_or(false);
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_fullscreen",
            "Get current fullscreen state",
            "render",
            &[],
            Some("boolean"),
        )?;

        engine.set(
            "get_vsync",
            self.lua.create_function(|lua, ()| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().vsync)
                    .unwrap_or(false);
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_vsync",
            "Get current vsync state",
            "render",
            &[],
            Some("boolean"),
        )?;

        engine.set(
            "get_target_fps",
            self.lua.create_function(|lua, ()| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.gameconfig_snapshot.borrow().target_fps)
                    .unwrap_or(60);
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_target_fps",
            "Get current target FPS",
            "render",
            &[],
            Some("integer"),
        )?;

        engine.set(
            "set_render_size",
            self.lua
                .create_function(|lua, (width, height): (u32, u32)| {
                    let width = width.clamp(320, 7680);
                    let height = height.clamp(200, 4320);
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .gameconfig_commands
                        .borrow_mut()
                        .push(GameConfigCmd::RenderSize { width, height });
                    Ok(())
                })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_render_size",
            "Set internal render resolution (min 320x200, max 7680x4320)",
            "render",
            &[("width", "integer"), ("height", "integer")],
            None,
        )?;

        engine.set(
            "get_render_size",
            self.lua.create_function(|lua, ()| {
                let (w, h) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.gameconfig_snapshot.borrow();
                        (snap.render_width, snap.render_height)
                    })
                    .unwrap_or((640, 360));
                let table = lua.create_table()?;
                table.set("width", w)?;
                table.set("height", h)?;
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_render_size",
            "Get current internal render resolution",
            "render",
            &[],
            Some("table"),
        )?;

        engine.set(
            "set_background_color",
            self.lua.create_function(|lua, (r, g, b): (u8, u8, u8)| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .gameconfig_commands
                    .borrow_mut()
                    .push(GameConfigCmd::BackgroundColor { r, g, b });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "set_background_color",
            "Set background clear color (RGB 0-255)",
            "render",
            &[("r", "integer"), ("g", "integer"), ("b", "integer")],
            None,
        )?;

        engine.set(
            "get_background_color",
            self.lua.create_function(|lua, ()| {
                let (r, g, b) = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| {
                        let snap = data.gameconfig_snapshot.borrow();
                        (snap.background_r, snap.background_g, snap.background_b)
                    })
                    .unwrap_or((80, 80, 80));
                let table = lua.create_table()?;
                table.set("r", r)?;
                table.set("g", g)?;
                table.set("b", b)?;
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_background_color",
            "Get current background clear color",
            "render",
            &[],
            Some("table"),
        )?;

        Ok(())
    }

    /// Registers the input rebinding API in the `engine` table.
    pub(super) fn register_input_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "rebind_action",
            input_commands,
            |(action, key)| (String, String),
            InputCmd::Rebind { action, key },
            desc = "Rebind a logical action to a new key (replaces existing binding)",
            cat = "input",
            params = [("action", "string"), ("key", "string")]
        );

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "add_binding",
            input_commands,
            |(action, key)| (String, String),
            InputCmd::AddBinding { action, key },
            desc = "Add an extra key binding for an action (supports multi-bind)",
            cat = "input",
            params = [("action", "string"), ("key", "string")]
        );

        engine.set(
            "get_binding",
            self.lua.create_function(|lua, action: String| {
                let result = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.bindings_snapshot.borrow().get(&action).cloned());
                Ok(result)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_binding",
            "Get the first key binding for an action as a string (nil if unbound)",
            "input",
            &[("action", "string")],
            Some("string?"),
        )?;

        Ok(())
    }
}
