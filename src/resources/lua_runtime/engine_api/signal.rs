use super::*;
use crate::resources::signal_keys as sk;

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        engine.set(
            "get_scalar",
            self.lua.create_function(|lua, key: LuaString| {
                let key = key.to_str()?;
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().scalars.get(&*key).copied());
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
            self.lua.create_function(|lua, key: LuaString| {
                let key = key.to_str()?;
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().integers.get(&*key).copied());
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
            self.lua.create_function(|lua, key: LuaString| {
                let key = key.to_str()?;
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().strings.get(&*key).cloned());
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
            self.lua.create_function(|lua, key: LuaString| {
                let key = key.to_str()?;
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_snapshot.borrow().flags.contains(&*key))
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
            self.lua.create_function(|lua, group: LuaString| {
                let group = group.to_str()?;
                let count = lua.app_data_ref::<LuaAppData>().and_then(|data| {
                    data.signal_snapshot
                        .borrow()
                        .group_counts
                        .get(&*group)
                        .copied()
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
            self.lua.create_function(|lua, key: LuaString| {
                let key = key.to_str()?;
                let entity_id = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().entities.get(&*key).copied());
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

        define_signal_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "",
            signal_commands,
            "signal",
            ""
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

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::{FxHashMap, FxHashSet};
    use std::sync::Arc;

    fn set_snapshot(runtime: &LuaRuntime, snapshot: crate::resources::worldsignals::SignalSnapshot) {
        if let Some(data) = runtime.lua().app_data_ref::<LuaAppData>() {
            *data.signal_snapshot.borrow_mut() = Arc::new(snapshot);
        }
    }

    #[test]
    fn get_scalar_returns_value_and_nil_for_missing_key() {
        let runtime = LuaRuntime::new().unwrap();
        let mut scalars = FxHashMap::default();
        scalars.insert("hp".to_string(), 42.0f32);
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                scalars: Arc::new(scalars),
                ..Default::default()
            },
        );

        let hp: Option<f32> = runtime
            .lua()
            .load("return engine.get_scalar('hp')")
            .eval()
            .unwrap();
        assert_eq!(hp, Some(42.0));

        let missing: Option<f32> = runtime
            .lua()
            .load("return engine.get_scalar('missing')")
            .eval()
            .unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn get_integer_returns_value_and_nil_for_missing_key() {
        let runtime = LuaRuntime::new().unwrap();
        let mut integers = FxHashMap::default();
        integers.insert("score".to_string(), 7);
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                integers: Arc::new(integers),
                ..Default::default()
            },
        );

        let score: Option<i32> = runtime
            .lua()
            .load("return engine.get_integer('score')")
            .eval()
            .unwrap();
        assert_eq!(score, Some(7));

        let missing: Option<i32> = runtime
            .lua()
            .load("return engine.get_integer('missing')")
            .eval()
            .unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn get_string_returns_value_and_nil_for_missing_key() {
        let runtime = LuaRuntime::new().unwrap();
        let mut strings = FxHashMap::default();
        strings.insert("name".to_string(), "Aberred".to_string());
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                strings: Arc::new(strings),
                ..Default::default()
            },
        );

        let name: Option<String> = runtime
            .lua()
            .load("return engine.get_string('name')")
            .eval()
            .unwrap();
        assert_eq!(name, Some("Aberred".to_string()));

        let missing: Option<String> = runtime
            .lua()
            .load("return engine.get_string('missing')")
            .eval()
            .unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn has_flag_true_for_set_flag_false_for_missing() {
        let runtime = LuaRuntime::new().unwrap();
        let mut flags = FxHashSet::default();
        flags.insert("paused".to_string());
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                flags: Arc::new(flags),
                ..Default::default()
            },
        );

        let paused: bool = runtime
            .lua()
            .load("return engine.has_flag('paused')")
            .eval()
            .unwrap();
        assert!(paused);

        let missing: bool = runtime
            .lua()
            .load("return engine.has_flag('missing')")
            .eval()
            .unwrap();
        assert!(!missing);
    }

    #[test]
    fn get_group_count_returns_value_and_nil_for_missing_key() {
        let runtime = LuaRuntime::new().unwrap();
        let mut group_counts = FxHashMap::default();
        group_counts.insert("enemies".to_string(), 3u32);
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                group_counts: Arc::new(group_counts),
                ..Default::default()
            },
        );

        let count: Option<u32> = runtime
            .lua()
            .load("return engine.get_group_count('enemies')")
            .eval()
            .unwrap();
        assert_eq!(count, Some(3));

        let missing: Option<u32> = runtime
            .lua()
            .load("return engine.get_group_count('missing')")
            .eval()
            .unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn get_entity_returns_value_and_nil_for_missing_key() {
        let runtime = LuaRuntime::new().unwrap();
        let mut entities = FxHashMap::default();
        entities.insert("player".to_string(), 99u64);
        set_snapshot(
            &runtime,
            crate::resources::worldsignals::SignalSnapshot {
                entities: Arc::new(entities),
                ..Default::default()
            },
        );

        let id: Option<u64> = runtime
            .lua()
            .load("return engine.get_entity('player')")
            .eval()
            .unwrap();
        assert_eq!(id, Some(99));

        let missing: Option<u64> = runtime
            .lua()
            .load("return engine.get_entity('missing')")
            .eval()
            .unwrap();
        assert_eq!(missing, None);
    }
}
