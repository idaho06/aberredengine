use super::super::entity_builder::LuaEntityBuilder;
use super::*;

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

        define_audio_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_audio_commands,
            "collision",
            " (collision context)"
        );

        define_signal_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_signal_commands,
            "collision",
            " (collision context)"
        );

        define_phase_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_phase_commands,
            "collision",
            " (collision context)"
        );

        define_camera_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "collision_",
            collision_camera_commands,
            "collision",
            " (collision context)"
        );

        register_getter!(engine, self.lua, meta_fns, "collision_spawn",
            |_lua, ()| { Ok(LuaEntityBuilder::new_collision()) },
            desc = "Create a new entity builder (collision context)", cat = "collision",
            params = [], returns = "CollisionEntityBuilder");

        // source_key is stored into the eventual SpawnCmd, not just read — stays an
        // owned String rather than converting to mlua::LuaString.
        register_getter!(engine, self.lua, meta_fns, "collision_clone",
            |_lua, source_key: String| { Ok(LuaEntityBuilder::new_collision_clone(source_key)) },
            desc = "Clone a registered entity (collision context)", cat = "collision",
            params = [("source_key", "string")], returns = "CollisionEntityBuilder");

        Ok(())
    }
}
