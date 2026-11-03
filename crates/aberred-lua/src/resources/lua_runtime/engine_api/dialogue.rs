//! `engine.register_dialogue` / `engine.load_dialogue` and the
//! `engine.entity_*dialogue*` entity commands.

use super::*;
use aberred_core::resources::dialogue_def::DialogueDef;
use aberred_core::resources::dialogue_tree::{DialogueTree, load_dialogue};
use std::sync::Arc;

impl LuaRuntime {
    /// Registers `engine.register_dialogue` and `engine.load_dialogue`.
    ///
    /// Both are fallible (the definition table is converted to a
    /// [`DialogueDef`] and validated into a [`DialogueTree`], or the JSON
    /// file at `path` is read with [`load_dialogue`] and validated), so they
    /// are hand-written `register_getter!` closures rather than
    /// `register_cmd!` rows. A conversion or validation failure raises a
    /// runtime error naming the key (and `path`) plus the underlying error,
    /// and queues nothing.
    pub(in crate::resources::lua_runtime) fn register_dialogue_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;
        let meta: LuaTable = engine.get("__meta")?;
        let meta_fns: LuaTable = meta.get("functions")?;
        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "register_dialogue",
            |lua, (key, def): (String, LuaTable)| -> LuaResult<()> {
                let def: DialogueDef = lua
                    .from_value(LuaValue::Table(def))
                    .map_err(|e| LuaError::runtime(format!("register_dialogue('{key}'): {e}")))?;
                let tree = DialogueTree::try_from(def)
                    .map_err(|e| LuaError::runtime(format!("register_dialogue('{key}'): {e}")))?;
                queue_register(lua, key, tree)
            },
            desc = "Register a dialogue definition table as a validated tree under `key`",
            cat = "dialogue",
            params = [("key", "string"), ("def", "table")]
        );
        register_getter!(
            engine,
            self.lua,
            meta_fns,
            "load_dialogue",
            |lua, (key, path): (String, String)| -> LuaResult<()> {
                let tree = load_dialogue(&path).map_err(|e| {
                    LuaError::runtime(format!("load_dialogue('{key}', '{path}'): {e}"))
                })?;
                queue_register(lua, key, tree)
            },
            desc = "Load a dialogue JSON file from `path` and register it under `key`",
            cat = "dialogue",
            params = [("key", "string"), ("path", "string")]
        );
        // Entity-level dialogue commands push plain `EntityCmd`s onto the
        // regular `entity_commands` queue; they are registered with
        // `register_cmd!` (not `define_entity_cmds!`) so no `collision_`
        // twins exist.
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "entity_insert_dialogue",
            entity_commands,
            |(entity_id, key)| (u64, String),
            EntityCmd::InsertDialogue { entity_id, key },
            desc = "Attach a DialogueRunner running the tree registered under `key`; an existing runner is replaced, an unknown key inserts nothing",
            cat = "dialogue",
            params = [("entity_id", "integer"), ("key", "string")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "entity_remove_dialogue",
            entity_commands,
            |entity_id| u64,
            EntityCmd::RemoveDialogue { entity_id },
            desc = "Remove the entity's DialogueRunner; no-op without one",
            cat = "dialogue",
            params = [("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "entity_dialogue_advance",
            entity_commands,
            |entity_id| u64,
            EntityCmd::DialogueAdvance { entity_id },
            desc = "Request advancing the entity's dialogue past the current node; no-op without a runner",
            cat = "dialogue",
            params = [("entity_id", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "entity_dialogue_choose",
            entity_commands,
            |(entity_id, index)| (u64, usize),
            EntityCmd::DialogueChoose { entity_id, index },
            desc = "Request picking the choice at 0-based declared `index` of the entity's current dialogue node; no-op without a runner",
            cat = "dialogue",
            params = [("entity_id", "integer"), ("index", "integer")]
        );
        register_cmd!(
            engine,
            self.lua,
            meta_fns,
            "entity_dialogue_cancel",
            entity_commands,
            |entity_id| u64,
            EntityCmd::DialogueCancel { entity_id },
            desc = "Request cancelling the entity's dialogue; no-op without a runner",
            cat = "dialogue",
            params = [("entity_id", "integer")]
        );
        Ok(())
    }
}

/// Queues a validated `tree` under `key` in the `dialogue_commands` queue.
fn queue_register(lua: &Lua, key: String, tree: DialogueTree) -> LuaResult<()> {
    let data = lua
        .app_data_ref::<LuaAppData>()
        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
    data.push(
        &data.dialogue_commands,
        DialogueLuaCmd::Register {
            key,
            tree: Arc::new(tree),
        },
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::super::entity_builder::test_helpers::assert_runtime_error_in;
    use super::*;
    use crate::resources::lua_runtime::{DialogueLuaCmd, EntityCmd};

    /// Drains the `dialogue_commands` queue into a fresh Vec.
    fn drain_dialogue(runtime: &LuaRuntime) -> Vec<DialogueLuaCmd> {
        let mut cmds = Vec::new();
        runtime.drain_dialogue_commands_into(&mut cmds);
        cmds
    }

    /// Runs `script` on a fresh runtime and returns the drained `dialogue_commands`.
    fn run_and_drain(script: &str) -> Vec<DialogueLuaCmd> {
        let runtime = LuaRuntime::new().unwrap();
        runtime.lua().load(script).exec().unwrap();
        drain_dialogue(&runtime)
    }

    /// A per-process temp JSON path unique to `test_name`.
    fn temp_json_path(test_name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("{}-{test_name}.json", std::process::id()))
    }

    #[test]
    fn register_dialogue_queues_a_validated_tree() {
        let cmds = run_and_drain(
            "engine.register_dialogue('k', { start = 'a', nodes = { { id = 'a', text = 'hi' } } })",
        );
        assert!(
            matches!(
                cmds.as_slice(),
                [DialogueLuaCmd::Register { key, tree }]
                    if key == "k" && tree.idx_of("a").is_some()
            ),
            "{cmds:?}"
        );
    }

    #[test]
    fn register_dialogue_round_trips_next_when_effects_on_enter_labels_and_speaker() {
        let cmds = run_and_drain(
            r#"engine.register_dialogue('k', {
                start = 'a',
                nodes = {
                    { id = 'a', speaker = 'S', text = 'hi', next = 'b',
                      on_enter = { { type = 'add_integer', key = 'gold', delta = -1 } },
                      labels = { 'x' } },
                    { id = 'b', text = 'bye', choices = {
                        { text = 't', next = 'a',
                          when = { type = 'flag', key = 'f' },
                          effects = { { type = 'set_flag', key = 'g' } } },
                        { text = 'end' } } },
                },
            })"#,
        );
        assert!(
            matches!(cmds.as_slice(), [DialogueLuaCmd::Register { key, .. }] if key == "k"),
            "{cmds:?}"
        );
    }

    #[test]
    fn register_dialogue_with_a_dangling_next_raises_and_queues_nothing() {
        let runtime = LuaRuntime::new().unwrap();
        let script = "engine.register_dialogue('quest', { start = 'a', nodes = { { id = 'a', text = 'hi', next = 'ghost' } } })";
        assert_runtime_error_in(&runtime, script, "ghost");
        assert_runtime_error_in(&runtime, script, "quest");
        assert!(drain_dialogue(&runtime).is_empty());
    }

    #[test]
    fn register_dialogue_with_a_malformed_table_raises_and_queues_nothing() {
        let runtime = LuaRuntime::new().unwrap();
        let script = "engine.register_dialogue('quest', { nodes = { { id = 'a', text = 'hi' } } })";
        assert_runtime_error_in(&runtime, script, "quest");
        assert!(drain_dialogue(&runtime).is_empty());
    }

    #[test]
    fn load_dialogue_registers_a_tree_from_a_json_file() {
        let path = temp_json_path("load_dialogue_registers_a_tree_from_a_json_file");
        std::fs::write(&path, r#"{"start":"a","nodes":[{"id":"a","text":"hi"}]}"#).unwrap();

        let cmds = run_and_drain(&format!(
            "engine.load_dialogue('k', '{}')",
            path.to_str().unwrap()
        ));
        let _ = std::fs::remove_file(&path);
        assert!(
            matches!(
                cmds.as_slice(),
                [DialogueLuaCmd::Register { key, tree }]
                    if key == "k" && tree.idx_of("a").is_some()
            ),
            "{cmds:?}"
        );
    }

    #[test]
    fn load_dialogue_with_a_missing_file_raises_naming_the_path() {
        let path = temp_json_path("load_dialogue_with_a_missing_file_raises_naming_the_path");
        let path_str = path.to_str().unwrap().to_owned();
        let runtime = LuaRuntime::new().unwrap();

        let err = runtime
            .lua()
            .load(format!("engine.load_dialogue('k', '{path_str}')"))
            .exec()
            .expect_err("a missing file must raise");

        assert!(
            err.to_string().contains(&path_str),
            "error names the path: {err}"
        );
        assert!(drain_dialogue(&runtime).is_empty());
    }

    #[test]
    fn registering_the_same_dialogue_key_twice_queues_two_commands_in_order() {
        let cmds = run_and_drain(
            "engine.register_dialogue('k', { start = 'a', nodes = { { id = 'a', text = 'hi' } } })\n\
             engine.register_dialogue('k', { start = 'a', nodes = { { id = 'a', text = 'yo' } } })",
        );
        assert!(
            matches!(
                cmds.as_slice(),
                [DialogueLuaCmd::Register { key: first, .. }, DialogueLuaCmd::Register { key: second, .. }]
                    if first == "k" && second == "k"
            ),
            "{cmds:?}"
        );
    }

    #[test]
    fn entity_dialogue_functions_queue_the_matching_entity_commands() {
        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load(
                "engine.entity_insert_dialogue(7, 'k')\n\
                 engine.entity_remove_dialogue(7)\n\
                 engine.entity_dialogue_advance(7)\n\
                 engine.entity_dialogue_choose(7, 2)\n\
                 engine.entity_dialogue_cancel(7)",
            )
            .exec()
            .unwrap();

        let mut queued = Vec::new();
        runtime.drain_entity_commands_into(&mut queued);
        let cmds: Vec<&EntityCmd> = queued.iter().map(|c| &c.cmd).collect();
        assert!(
            matches!(
                cmds.as_slice(),
                [
                    EntityCmd::InsertDialogue { entity_id: 7, key },
                    EntityCmd::RemoveDialogue { entity_id: 7 },
                    EntityCmd::DialogueAdvance { entity_id: 7 },
                    EntityCmd::DialogueChoose { entity_id: 7, index: 2 },
                    EntityCmd::DialogueCancel { entity_id: 7 },
                ] if key == "k"
            ),
            "{cmds:?}"
        );
    }

    #[test]
    fn entity_dialogue_functions_have_no_collision_twins() {
        let runtime = LuaRuntime::new().unwrap();
        let engine: LuaTable = runtime.lua().globals().get("engine").unwrap();
        for name in [
            "entity_insert_dialogue",
            "entity_remove_dialogue",
            "entity_dialogue_advance",
            "entity_dialogue_choose",
            "entity_dialogue_cancel",
        ] {
            assert!(
                matches!(engine.get::<LuaValue>(name), Ok(LuaValue::Function(_))),
                "engine.{name} is registered"
            );
            let twin = format!("collision_{name}");
            assert!(
                matches!(engine.get::<LuaValue>(twin.as_str()), Ok(LuaValue::Nil)),
                "engine.{twin} is nil"
            );
        }
    }
}
