use super::*;
use aberred_core::components::signals::Signals;
use aberred_core::resources::signal_text::{format_signal_text, validate_signal_text};

impl LuaRuntime {
    pub(in crate::resources::lua_runtime) fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_scalar",
            |lua, key: LuaString| {
                let key = key.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().scalars.get(&*key).copied()))
            },
            desc = "Get a world signal scalar value",
            cat = "signal",
            params = [("key", "string")],
            returns = "number?"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_integer",
            |lua, key: LuaString| {
                let key = key.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().integers.get(&*key).copied()))
            },
            desc = "Get a world signal integer value",
            cat = "signal",
            params = [("key", "string")],
            returns = "integer?"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_string",
            |lua, key: LuaString| {
                let key = key.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().strings.get(&*key).cloned()))
            },
            desc = "Get a world signal string value",
            cat = "signal",
            params = [("key", "string")],
            returns = "string?"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "format_text",
            |lua, (template, signals): (LuaString, Option<LuaTable>)| {
                format_text(lua, &template.to_str()?, signals.as_ref())
            },
            desc = "Format `template`, filling `{key}` / `{self.key}` / `{key:.N}` placeholders. World values come from the per-tick signal snapshot, so an `engine.set_*` queued earlier in the same callback is not visible yet; `{self.key}` reads the optional `signals` table (normally a callback's `ctx.signals`), whose values are frozen at the time that table was built",
            cat = "signal",
            params = [("template", "string"), ("signals", "table?")],
            returns = "string"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "has_flag",
            |lua, key: LuaString| {
                let key = key.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_snapshot.borrow().flags.contains(&*key))
                    .unwrap_or(false))
            },
            desc = "Check if a world signal flag is set",
            cat = "signal",
            params = [("key", "string")],
            returns = "boolean"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_group_count",
            |lua, group: LuaString| {
                let group = group.to_str()?;
                Ok(lua.app_data_ref::<LuaAppData>().and_then(|data| {
                    data.signal_snapshot
                        .borrow()
                        .group_counts
                        .get(&*group)
                        .copied()
                }))
            },
            desc = "Get the count of entities in a tracked group",
            cat = "signal",
            params = [("group", "string")],
            returns = "integer?"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_entity",
            |lua, key: LuaString| {
                let key = key.to_str()?;
                Ok(lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_snapshot.borrow().entities.get(&*key).copied()))
            },
            desc = "Get a registered entity ID by key",
            cat = "signal",
            params = [("key", "string")],
            returns = "integer?"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_scalars",
            |lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.scalars.iter() {
                        table.set(key.as_str(), *value)?;
                    }
                }
                Ok(table)
            },
            desc = "Get all world signal scalars as a snapshot table",
            cat = "signal",
            params = [],
            returns = "table"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_integers",
            |lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.integers.iter() {
                        table.set(key.as_str(), *value)?;
                    }
                }
                Ok(table)
            },
            desc = "Get all world signal integers as a snapshot table",
            cat = "signal",
            params = [],
            returns = "table"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_strings",
            |lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (key, value) in snapshot.strings.iter() {
                        table.set(key.as_str(), value.as_str())?;
                    }
                }
                Ok(table)
            },
            desc = "Get all world signal strings as a snapshot table",
            cat = "signal",
            params = [],
            returns = "table"
        );

        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "get_flags",
            |lua, ()| {
                let table = lua.create_table()?;
                if let Some(data) = lua.app_data_ref::<LuaAppData>() {
                    let snapshot = data.signal_snapshot.borrow();
                    for (index, flag) in snapshot.flags.iter().enumerate() {
                        table.set(index + 1, flag.as_str())?;
                    }
                }
                Ok(table)
            },
            desc = "Get all world signal flags as a snapshot array",
            cat = "signal",
            params = [],
            returns = "table"
        );

        define_signal_cmd_twins!(
            engine,
            self.lua,
            meta_fns,
            "",
            signal_commands,
            "signal",
            ""
        );

        // A request (NextScene), applied by the next scene switch poll; the
        // scene string stays put until then.
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "request_scene",
            signal_commands,
            |name| String,
            SignalCmd::RequestScene { name },
            desc = "Request a switch to scene `name`, applied by the next scene switch poll",
            cat = "base",
            params = [("name", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "request_quit",
            signal_commands,
            |()| (),
            SignalCmd::RequestQuit,
            desc = "Request quitting the game (sets the Quitting state as the next game state)",
            cat = "base",
            params = []
        );

        Ok(())
    }
}

/// `engine.format_text` implementation: fills the `{key}` / `{self.key}` / `{key:.N}`
/// placeholders of `template` through the core [`format_signal_text`].
///
/// World values come from the per-tick signal snapshot, so an `engine.set_*` queued earlier in
/// the same callback is not visible yet. `{self.key}` reads `signals` (normally a callback's
/// `ctx.signals`), whose values are frozen at the time that table was built, not live.
fn format_text(lua: &Lua, template: &str, signals: Option<&LuaTable>) -> LuaResult<String> {
    if let Err(err) = validate_signal_text(template) {
        return Err(LuaError::runtime(format!(
            "format_text: invalid template: {err}"
        )));
    }
    let data = lua
        .app_data_ref::<LuaAppData>()
        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
    let snapshot = data.signal_snapshot.borrow();
    let entity = signals.map(entity_signals_from_table).transpose()?;
    Ok(format_signal_text(template, &**snapshot, entity.as_ref()).into_owned())
}

/// Builds the entity [`Signals`] that `{self.key}` reads from a `signals` table shaped like a
/// callback's `ctx.signals`: `flags` is a sequence of strings, `integers` / `scalars` / `strings`
/// are string-keyed maps, and any of the four sub-tables may be absent. A sub-table key holding
/// the wrong value type fails with the normal mlua conversion error.
fn entity_signals_from_table(table: &LuaTable) -> LuaResult<Signals> {
    let mut signals = Signals::default();
    if let Some(flags) = table.get::<Option<LuaTable>>("flags")? {
        for pair in flags.pairs::<i64, String>() {
            signals.set_flag(pair?.1);
        }
    }
    if let Some(integers) = table.get::<Option<LuaTable>>("integers")? {
        for pair in integers.pairs::<String, i32>() {
            let (key, value) = pair?;
            signals.set_integer(key, value);
        }
    }
    if let Some(scalars) = table.get::<Option<LuaTable>>("scalars")? {
        for pair in scalars.pairs::<String, f32>() {
            let (key, value) = pair?;
            signals.set_scalar(key, value);
        }
    }
    if let Some(strings) = table.get::<Option<LuaTable>>("strings")? {
        for pair in strings.pairs::<String, String>() {
            let (key, value) = pair?;
            signals.set_string(key, value);
        }
    }
    Ok(signals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_hash::{FxHashMap, FxHashSet};
    use std::sync::Arc;

    fn set_snapshot(
        runtime: &LuaRuntime,
        snapshot: aberred_core::resources::world_signals::SignalSnapshot,
    ) {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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
            aberred_core::resources::world_signals::SignalSnapshot {
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

    fn world_snapshot(
        integers: &[(&str, i32)],
        scalars: &[(&str, f32)],
        strings: &[(&str, &str)],
    ) -> aberred_core::resources::world_signals::SignalSnapshot {
        aberred_core::resources::world_signals::SignalSnapshot {
            integers: Arc::new(integers.iter().map(|(k, v)| (k.to_string(), *v)).collect()),
            scalars: Arc::new(scalars.iter().map(|(k, v)| (k.to_string(), *v)).collect()),
            strings: Arc::new(
                strings
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            ..Default::default()
        }
    }

    #[test]
    fn format_text_fills_world_keys() {
        let runtime = LuaRuntime::new().unwrap();
        set_snapshot(
            &runtime,
            world_snapshot(&[("apples", 6)], &[], &[("player_name", "Idaho06")]),
        );

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{player_name} has {apples}')")
            .eval()
            .unwrap();
        assert_eq!(out, "Idaho06 has 6");
    }

    #[test]
    fn format_text_reads_self_keys_from_signals_table() {
        let runtime = LuaRuntime::new().unwrap();

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{self.apples}', {integers = {apples = 4}})")
            .eval()
            .unwrap();
        assert_eq!(out, "4");

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{self.ratio}', {scalars = {ratio = 1.5}})")
            .eval()
            .unwrap();
        assert_eq!(out, "1.5");

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{self.who}', {strings = {who = 'bot'}})")
            .eval()
            .unwrap();
        assert_eq!(out, "bot");

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{self.open}', {flags = {'open'}})")
            .eval()
            .unwrap();
        assert_eq!(out, "true");
    }

    #[test]
    fn format_text_without_table_keeps_self_token_literal() {
        let runtime = LuaRuntime::new().unwrap();

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{self.apples}')")
            .eval()
            .unwrap();
        assert_eq!(out, "{self.apples}");
    }

    #[test]
    fn format_text_reflects_snapshot_change_on_next_call() {
        let runtime = LuaRuntime::new().unwrap();
        set_snapshot(&runtime, world_snapshot(&[("apples", 6)], &[], &[]));

        let first: String = runtime
            .lua()
            .load("return engine.format_text('{apples}')")
            .eval()
            .unwrap();
        assert_eq!(first, "6");

        set_snapshot(&runtime, world_snapshot(&[("apples", 9)], &[], &[]));

        let second: String = runtime
            .lua()
            .load("return engine.format_text('{apples}')")
            .eval()
            .unwrap();
        assert_eq!(second, "9");
    }

    #[test]
    fn format_text_applies_precision_to_numbers() {
        let runtime = LuaRuntime::new().unwrap();
        set_snapshot(&runtime, world_snapshot(&[], &[("grain", 2.5)], &[]));

        let out: String = runtime
            .lua()
            .load("return engine.format_text('{grain:.2}')")
            .eval()
            .unwrap();
        assert_eq!(out, "2.50");
    }

    #[test]
    fn format_text_rejects_malformed_template() {
        let runtime = LuaRuntime::new().unwrap();

        let err = runtime
            .lua()
            .load("return engine.format_text('{')")
            .eval::<String>()
            .unwrap_err();
        assert!(
            err.to_string().contains("format_text"),
            "error should name format_text: {err}"
        );
    }

    #[test]
    fn format_text_keeps_literal_token_for_missing_world_key() {
        let runtime = LuaRuntime::new().unwrap();

        let out: String = runtime
            .lua()
            .load("return engine.format_text('a {missing} b')")
            .eval()
            .unwrap();
        assert_eq!(out, "a {missing} b");
    }
}
