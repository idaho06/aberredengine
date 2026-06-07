use super::*;
use super::super::entity_builder::LuaEntityBuilder;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_spawn_api(&self) -> LuaResult<()> {
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
}