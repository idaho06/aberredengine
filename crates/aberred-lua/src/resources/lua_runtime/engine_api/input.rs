use super::*;
use crate::resources::lua_runtime::action_from_str;
use aberred_core::events::input::InputAction;

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

        register_getter!(engine, self.lua, meta_fns, "get_binding",
            |lua, action: LuaString| {
                let s = action.to_str()?;
                let canonical = action_from_str(&s).map(InputAction::name).unwrap_or(&s);
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.bindings_snapshot.borrow().get(canonical).cloned()))
            },
            desc = "Get the first key binding for an action as a string (nil if unbound)", cat = "input",
            params = [("action", "string")], returns = "string?");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_binding(runtime: &LuaRuntime, action: &str, key: &str) {
        if let Some(data) = runtime.lua().app_data_ref::<LuaAppData>() {
            data.bindings_snapshot
                .borrow_mut()
                .insert(action.to_string(), key.to_string());
        }
    }

    #[test]
    fn get_binding_resolves_canonical_and_alias_action_names() {
        let runtime = LuaRuntime::new().unwrap();
        set_binding(&runtime, "main_up", "W");

        let canonical: Option<String> = runtime
            .lua()
            .load("return engine.get_binding('main_up')")
            .eval()
            .unwrap();
        assert_eq!(canonical, Some("W".to_string()));

        // "up" is an alias for "main_up" (action_from_str), so it must resolve
        // to the same canonical binding.
        let alias: Option<String> = runtime
            .lua()
            .load("return engine.get_binding('up')")
            .eval()
            .unwrap();
        assert_eq!(alias, Some("W".to_string()));
    }

    #[test]
    fn get_binding_falls_back_to_raw_string_for_unknown_action() {
        let runtime = LuaRuntime::new().unwrap();
        // Not a recognized InputAction name, so action_from_str returns None
        // and get_binding must fall back to looking up the raw string itself.
        set_binding(&runtime, "custom_action", "X");

        let result: Option<String> = runtime
            .lua()
            .load("return engine.get_binding('custom_action')")
            .eval()
            .unwrap();
        assert_eq!(result, Some("X".to_string()));
    }

    #[test]
    fn get_binding_returns_nil_for_unbound_action() {
        let runtime = LuaRuntime::new().unwrap();
        let result: Option<String> = runtime
            .lua()
            .load("return engine.get_binding('main_up')")
            .eval()
            .unwrap();
        assert_eq!(result, None);
    }

    /// Runs `script`, then applies every queued input command to `bindings`.
    fn apply_input_script(
        script: &str,
        bindings: &mut aberred_core::resources::input_bindings::InputBindings,
    ) {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        let mut cmds = Vec::new();
        runtime.drain_input_commands_into(&mut cmds);
        for cmd in cmds {
            crate::systems::lua_commands::process_input_command(cmd, bindings);
        }
    }

    #[test]
    fn rebind_replaces_and_add_binding_appends() {
        use aberred_core::events::input::InputAction;
        use aberred_core::resources::input_bindings::{InputBindings, binding_from_str};
        let mut bindings = InputBindings::default();

        apply_input_script("engine.rebind_action('action_1', 'z')", &mut bindings);
        assert_eq!(
            bindings.get_bindings(InputAction::Action1),
            [binding_from_str("z").unwrap()],
            "rebind leaves exactly one binding"
        );

        apply_input_script("engine.add_binding('action_1', 'x')", &mut bindings);
        assert_eq!(
            bindings.get_bindings(InputAction::Action1),
            [binding_from_str("z").unwrap(), binding_from_str("x").unwrap()]
        );
    }

    #[test]
    fn unknown_action_or_key_leaves_bindings_unchanged() {
        use aberred_core::resources::input_bindings::InputBindings;
        let mut bindings = InputBindings::default();
        let before = format!("{:?}", bindings.iter().collect::<Vec<_>>());

        apply_input_script(
            "engine.rebind_action('jump', 'z') engine.rebind_action('action_1', 'no_such_key') \
             engine.add_binding('jump', 'z') engine.add_binding('action_1', 'no_such_key')",
            &mut bindings,
        );
        assert_eq!(format!("{:?}", bindings.iter().collect::<Vec<_>>()), before);
    }
}
