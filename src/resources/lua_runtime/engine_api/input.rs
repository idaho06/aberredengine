use super::*;

impl LuaRuntime {
    /// Registers the input rebinding API in the `engine` table.
    pub(in crate::resources::lua_runtime) fn register_input_api(&self) -> LuaResult<()> {
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