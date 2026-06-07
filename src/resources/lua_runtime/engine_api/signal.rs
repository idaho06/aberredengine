use super::*;
use crate::resources::signal_keys as sk;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "get_scalar",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().scalars.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_scalar",
            "Get a world signal scalar value",
            "signal",
            &[("key", "string")],
            Some("number?"),
        )?;

        engine.set(
            "get_integer",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().integers.get(&key).copied());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_integer",
            "Get a world signal integer value",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        engine.set(
            "get_string",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().strings.get(&key).cloned());
                Ok(value)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_string",
            "Get a world signal string value",
            "signal",
            &[("key", "string")],
            Some("string?"),
        )?;

        engine.set(
            "has_flag",
            self.lua.create_function(|lua, key: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_snapshot.borrow().flags.contains(&key))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "has_flag",
            "Check if a world signal flag is set",
            "signal",
            &[("key", "string")],
            Some("boolean"),
        )?;

        engine.set(
            "get_group_count",
            self.lua.create_function(|lua, group: String| {
                let count = lua.app_data_ref::<LuaAppData>().and_then(|data| {
                    data.signal_snapshot.borrow().group_counts.get(&group).copied()
                });
                Ok(count)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_group_count",
            "Get the count of entities in a tracked group",
            "signal",
            &[("group", "string")],
            Some("integer?"),
        )?;

        engine.set(
            "get_entity",
            self.lua.create_function(|lua, key: String| {
                let entity_id = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().entities.get(&key).copied());
                Ok(entity_id)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_entity",
            "Get a registered entity ID by key",
            "signal",
            &[("key", "string")],
            Some("integer?"),
        )?;

        engine.set(
            "get_scalars",
            self.lua.create_function(|lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.scalars.iter() {
                        table.set(key.as_str(), *value)?;
                    }
                }
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_scalars",
            "Get all world signal scalars as a snapshot table",
            "signal",
            &[],
            Some("table"),
        )?;

        engine.set(
            "get_integers",
            self.lua.create_function(|lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.integers.iter() {
                        table.set(key.as_str(), *value)?;
                    }
                }
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_integers",
            "Get all world signal integers as a snapshot table",
            "signal",
            &[],
            Some("table"),
        )?;

        engine.set(
            "get_strings",
            self.lua.create_function(|lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.strings.iter() {
                        table.set(key.as_str(), value.as_str())?;
                    }
                }
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_strings",
            "Get all world signal strings as a snapshot table",
            "signal",
            &[],
            Some("table"),
        )?;

        engine.set(
            "get_flags",
            self.lua.create_function(|lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (index, flag) in snapshot.flags.iter().enumerate() {
                        table.set(index + 1, flag.as_str())?;
                    }
                }
                Ok(table)
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "get_flags",
            "Get all world signal flags as a snapshot array",
            "signal",
            &[],
            Some("table"),
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_scalar",
            signal_commands,
            |(key, value)| (String, f32),
            SignalCmd::SetScalar { key, value },
            desc = "Set a world signal scalar value",
            cat = "signal",
            params = [("key", "string"), ("value", "number")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_integer",
            signal_commands,
            |(key, value)| (String, i32),
            SignalCmd::SetInteger { key, value },
            desc = "Set a world signal integer value",
            cat = "signal",
            params = [("key", "string"), ("value", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_string",
            signal_commands,
            |(key, value)| (String, String),
            SignalCmd::SetString { key, value },
            desc = "Set a world signal string value",
            cat = "signal",
            params = [("key", "string"), ("value", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_flag",
            signal_commands,
            |key| String,
            SignalCmd::SetFlag { key },
            desc = "Set a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );

        engine.set(
            "change_scene",
            self.lua.create_function(|lua, scene_name: String| {
                let data = lua
                    .app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                let mut cmds = data.signal_commands.borrow_mut();
                cmds.push(SignalCmd::SetString {
                    key: sk::SCENE.into(),
                    value: scene_name,
                });
                cmds.push(SignalCmd::SetFlag {
                    key: sk::SWITCH_SCENE.into(),
                });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "change_scene",
            "Switch to a new scene by name (sets scene string + switch_scene flag)",
            "base",
            &[("scene_name", "string")],
            None,
        )?;

        engine.set(
            "quit",
            self.lua.create_function(|lua, ()| {
                let data = lua
                    .app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                data.signal_commands.borrow_mut().push(SignalCmd::SetFlag {
                    key: sk::QUIT_GAME.into(),
                });
                Ok(())
            })?,
        )?;
        push_fn_meta(
            &self.lua,
            &meta_fns,
            "quit",
            "Quit the game engine (sets quit_game flag)",
            "base",
            &[],
            None,
        )?;

        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_flag",
            signal_commands,
            |key| String,
            SignalCmd::ClearFlag { key },
            desc = "Clear a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "toggle_flag",
            signal_commands,
            |key| String,
            SignalCmd::ToggleFlag { key },
            desc = "Toggle a world signal flag",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_scalar",
            signal_commands,
            |key| String,
            SignalCmd::ClearScalar { key },
            desc = "Clear a world signal scalar",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_integer",
            signal_commands,
            |key| String,
            SignalCmd::ClearInteger { key },
            desc = "Clear a world signal integer",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "clear_string",
            signal_commands,
            |key| String,
            SignalCmd::ClearString { key },
            desc = "Clear a world signal string",
            cat = "signal",
            params = [("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "set_entity",
            signal_commands,
            |(key, entity_id)| (String, u64),
            SignalCmd::SetEntity { key, entity_id },
            desc = "Register an entity ID in world signals",
            cat = "signal",
            params = [("key", "string"), ("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "remove_entity",
            signal_commands,
            |key| String,
            SignalCmd::RemoveEntity { key },
            desc = "Remove a registered entity from world signals",
            cat = "signal",
            params = [("key", "string")]
        );

        Ok(())
    }
}