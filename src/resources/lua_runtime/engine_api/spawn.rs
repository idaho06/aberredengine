use super::super::entity_builder::LuaEntityBuilder;
use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_spawn_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_getter!(engine, self.lua, meta_fns, "spawn",
            |_lua, ()| () { Ok(LuaEntityBuilder::new()) },
            desc = "Create a new entity builder", cat = "spawn",
            params = [], returns = "EntityBuilder");

        // source_key is stored into the eventual SpawnCmd, not just read — stays an
        // owned String rather than converting to mlua::LuaString.
        register_getter!(engine, self.lua, meta_fns, "clone",
            |_lua, source_key| String { Ok(LuaEntityBuilder::new_clone(source_key)) },
            desc = "Clone a registered entity with optional overrides", cat = "spawn",
            params = [("source_key", "string")], returns = "EntityBuilder");

        Ok(())
    }
}
