use mlua::prelude::*;

/// Pushes function metadata to `engine.__meta.functions[name]`.
pub(super) fn push_fn_meta(
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

/// Registers one of the `engine.log_*` functions.
macro_rules! register_log_fn {
    ($engine:expr, $lua:expr, $meta_fns:expr, $name:expr, $log_macro:ident, $desc:expr) => {
        $engine.set(
            $name,
            $lua.create_function(|_, msg: String| {
                $log_macro!(target: "lua", "{}", msg);
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &$lua,
            &$meta_fns,
            $name,
            $desc,
            "base",
            &[("message", "string")],
            None,
        )?;
    };
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

/// Registers a declarative list of commands under `$queue`/`$cat`, with
/// each Lua function name prefixed by `$prefix` and each description
/// suffixed by `$desc_suffix`. Call once per context (regular / collision)
/// with the same list to keep both twins in sync.
macro_rules! define_cmd_twins {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, $cat:expr, $desc_suffix:literal, [
        $( ($name:literal,
            |$args:pat_param| $arg_ty:ty, $cmd:expr,
            desc = $desc:expr,
            params = [ $( ($pname:expr, $pty:expr) ),* $(,)? ]
        ) ),* $(,)?
    ]) => {
        $(
            register_cmd!($engine, $lua, $meta_fns, concat!($prefix, $name), $queue,
                |$args| $arg_ty, $cmd,
                desc = concat!($desc, $desc_suffix), cat = $cat,
                params = [ $( ($pname, $pty) ),* ]);
        )*
    };
}

/// Signal-command twins (regular `signal_commands` / `collision_signal_commands`).
macro_rules! define_signal_cmd_twins {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, $cat:expr, $desc_suffix:literal) => {
        define_cmd_twins!($engine, $lua, $meta_fns, $prefix, $queue, $cat, $desc_suffix, [
            ("set_scalar", |(key, value)| (String, f32), SignalCmd::SetScalar { key, value },
                desc = "Set a world signal scalar value",
                params = [("key", "string"), ("value", "number")]),
            ("set_integer", |(key, value)| (String, i32), SignalCmd::SetInteger { key, value },
                desc = "Set a world signal integer value",
                params = [("key", "string"), ("value", "integer")]),
            ("set_string", |(key, value)| (String, String), SignalCmd::SetString { key, value },
                desc = "Set a world signal string value",
                params = [("key", "string"), ("value", "string")]),
            ("set_flag", |key| String, SignalCmd::SetFlag { key },
                desc = "Set a world signal flag",
                params = [("key", "string")]),
            ("clear_flag", |key| String, SignalCmd::ClearFlag { key },
                desc = "Clear a world signal flag",
                params = [("key", "string")]),
            ("toggle_flag", |key| String, SignalCmd::ToggleFlag { key },
                desc = "Toggle a world signal flag",
                params = [("key", "string")]),
            ("clear_scalar", |key| String, SignalCmd::ClearScalar { key },
                desc = "Clear a world signal scalar",
                params = [("key", "string")]),
            ("clear_integer", |key| String, SignalCmd::ClearInteger { key },
                desc = "Clear a world signal integer",
                params = [("key", "string")]),
            ("clear_string", |key| String, SignalCmd::ClearString { key },
                desc = "Clear a world signal string",
                params = [("key", "string")]),
            ("set_entity", |(key, entity_id)| (String, u64), SignalCmd::SetEntity { key, entity_id },
                desc = "Register an entity ID in world signals",
                params = [("key", "string"), ("entity_id", "integer")]),
            ("remove_entity", |key| String, SignalCmd::RemoveEntity { key },
                desc = "Remove a registered entity from world signals",
                params = [("key", "string")]),
        ]);
    };
}

/// Camera-command twins (regular `camera_commands` / `collision_camera_commands`).
macro_rules! define_camera_cmd_twins {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, $cat:expr, $desc_suffix:literal) => {
        define_cmd_twins!($engine, $lua, $meta_fns, $prefix, $queue, $cat, $desc_suffix, [
            ("set_camera",
                |(target_x, target_y, offset_x, offset_y, rotation, zoom)| (f32, f32, f32, f32, f32, f32),
                CameraCmd::SetCamera2D { target_x, target_y, offset_x, offset_y, rotation, zoom },
                desc = "Set the 2D camera target, offset, rotation and zoom",
                params = [
                    ("target_x", "number"),
                    ("target_y", "number"),
                    ("offset_x", "number"),
                    ("offset_y", "number"),
                    ("rotation", "number"),
                    ("zoom", "number")
                ]),
        ]);
    };
}

/// Audio-command twins (regular `audio_commands` / `collision_audio_commands`).
macro_rules! define_audio_cmd_twins {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, $cat:expr, $desc_suffix:literal) => {
        define_cmd_twins!($engine, $lua, $meta_fns, $prefix, $queue, $cat, $desc_suffix, [
            ("play_sound", |id| String, AudioLuaCmd::PlaySound { id },
                desc = "Play a sound effect",
                params = [("id", "string")]),
            ("play_sound_pitched", |(id, pitch)| (String, f32), AudioLuaCmd::PlaySoundPitched { id, pitch },
                desc = "Play a sound effect with pitch override (1.0 = normal)",
                params = [("id", "string"), ("pitch", "number")]),
        ]);
    };
}

/// Phase-command twins (regular `phase_commands` / `collision_phase_commands`).
macro_rules! define_phase_cmd_twins {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident, $cat:expr, $desc_suffix:literal) => {
        define_cmd_twins!($engine, $lua, $meta_fns, $prefix, $queue, $cat, $desc_suffix, [
            ("phase_transition", |(entity_id, phase)| (u64, String), PhaseCmd::TransitionTo { entity_id, phase },
                desc = "Transition an entity to a new phase",
                params = [("entity_id", "integer"), ("phase", "string")]),
        ]);
    };
}

/// Defines and registers all entity commands for a given prefix and queue.
/// Called with `""` prefix for regular commands, `"collision_"` for collision commands.
macro_rules! define_entity_cmds {
    ($engine:expr, $lua:expr, $meta_fns:expr, $prefix:literal, $queue:ident) => {
        define_cmd_twins!($engine, $lua, $meta_fns, $prefix, $queue, "entity", "", [
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
            ("entity_signal_toggle_flag",
                |(entity_id, flag)| (u64, String), EntityCmd::SignalToggleFlag { entity_id, flag },
                desc = "Toggle a flag on an entity's signals",
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
            ("entity_insert_tween_screen_position",
                |(entity_id, from_x, from_y, to_x, to_y, duration, easing, loop_mode, backwards)|
                (u64, f32, f32, f32, f32, f32, String, String, bool),
                EntityCmd::InsertTweenScreenPosition {
                    entity_id, from_x, from_y, to_x, to_y,
                    config: TweenConfig { duration, easing, loop_mode, backwards },
                },
                desc = "Insert a screen-position tween on an entity (also inserts ScreenPosition itself if missing)",
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
            ("entity_signal_clear_scalar",
                |(entity_id, key)| (u64, String),
                EntityCmd::SignalClearScalar { entity_id, key },
                desc = "Clear a scalar signal on an entity",
                params = [("entity_id", "integer"), ("key", "string")]),
            ("entity_signal_set_string",
                |(entity_id, key, value)| (u64, String, String),
                EntityCmd::SignalSetString { entity_id, key, value },
                desc = "Set a string signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "string")]),
            ("entity_signal_clear_string",
                |(entity_id, key)| (u64, String),
                EntityCmd::SignalClearString { entity_id, key },
                desc = "Clear a string signal on an entity",
                params = [("entity_id", "integer"), ("key", "string")]),
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
            ("entity_remove_screen_position", |entity_id| u64, EntityCmd::RemoveScreenPosition { entity_id },
                desc = "Remove ScreenPosition from an entity",
                params = [("entity_id", "integer")]),
            ("entity_signal_set_integer",
                |(entity_id, key, value)| (u64, String, i32),
                EntityCmd::SignalSetInteger { entity_id, key, value },
                desc = "Set an integer signal on an entity",
                params = [("entity_id", "integer"), ("key", "string"), ("value", "integer")]),
            ("entity_signal_clear_integer",
                |(entity_id, key)| (u64, String),
                EntityCmd::SignalClearInteger { entity_id, key },
                desc = "Clear an integer signal on an entity",
                params = [("entity_id", "integer"), ("key", "string")]),
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
                |(entity_id, priority, zoom)| (u64, Option<u8>, Option<f32>),
                EntityCmd::SetCameraTarget { entity_id, priority, zoom },
                desc = "Set CameraTarget component on an entity (higher priority wins). Omitted priority/zoom preserve the entity's existing value (or component defaults if none exists); zoom is smoothly lerped each frame via zoom_lerp_speed",
                params = [("entity_id", "integer"), ("priority", "integer?"), ("zoom", "number?")]),
            ("entity_remove_camera_target", |entity_id| u64,
                EntityCmd::RemoveCameraTarget { entity_id },
                desc = "Remove CameraTarget component from an entity",
                params = [("entity_id", "integer")]),
        ]);
    };
}