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