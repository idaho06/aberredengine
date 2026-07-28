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

        register_getter!(engine, self.lua, meta_fns, "has_tracked_group",
            |lua, name| LuaString {
                let name = name.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.tracked_groups.borrow().contains(&*name))
                    .unwrap_or(false))
            },
            desc = "Check if a group is being tracked", cat = "group",
            params = [("name", "string")], returns = "boolean");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_tracked_group_true_for_tracked_false_for_missing() {
        let runtime = LuaRuntime::new().unwrap();
        if let Some(data) = runtime.lua().app_data_ref::<LuaAppData>() {
            data.tracked_groups
                .borrow_mut()
                .insert("enemies".to_string());
        }

        let tracked: bool = runtime
            .lua()
            .load("return engine.has_tracked_group('enemies')")
            .eval()
            .unwrap();
        assert!(tracked);

        let missing: bool = runtime
            .lua()
            .load("return engine.has_tracked_group('missing')")
            .eval()
            .unwrap();
        assert!(!missing);
    }
}
