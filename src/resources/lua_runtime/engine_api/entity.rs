use super::*;
use super::super::entity_builder::LuaEntityBuilder;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_entity_cmds!(engine, self.lua, meta_fns, "", entity_commands);
        Ok(())
    }

    pub(in crate::resources::lua_runtime) fn register_collision_api(&self) -> LuaResult<()> {
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
            "collision_toggle_flag",
            collision_signal_commands,
            |flag| String,
            SignalCmd::ToggleFlag { key: flag },
            desc = "Toggle a world signal flag (collision context)",
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
            "collision_set_entity",
            collision_signal_commands,
            |(key, entity_id)| (String, u64),
            SignalCmd::SetEntity { key, entity_id },
            desc = "Register an entity ID in world signals (collision context)",
            cat = "collision",
            params = [("key", "string"), ("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "collision_remove_entity",
            collision_signal_commands,
            |key| String,
            SignalCmd::RemoveEntity { key },
            desc = "Remove a registered entity from world signals (collision context)",
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
}