use super::*;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        define_phase_cmd_twins!(engine, self.lua, meta_fns, "", phase_commands, "phase", "");
        Ok(())
    }

    pub(in crate::resources::lua_runtime) fn register_group_api(&self) -> LuaResult<()> {
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
}