//! `engine.entity_insert_tag` / `engine.entity_remove_tag` and their `collision_` twins.

use super::super::lua_tag::{LuaTag, resolve_lua_tag};
use super::*;

/// Builds the queued command from the entity id and the resolved tag.
type MakeTagCmd = fn(u64, LuaTag) -> EntityCmd;

impl LuaRuntime {
    /// Registers `entity_insert_tag` / `entity_remove_tag` (`collision_`-prefixed and pushing
    /// to the collision queue when `collision`).
    ///
    /// These are hand-written rather than `define_entity_cmds!` rows because a row's command
    /// expression cannot reach the Lua state to resolve the tag name.
    pub(in crate::resources::lua_runtime) fn register_tag_cmds(
        &self,
        engine: &LuaTable,
        meta_fns: &LuaTable,
        collision: bool,
    ) -> LuaResult<()> {
        let prefix = if collision { "collision_" } else { "" };
        let fns: [(&str, &str, MakeTagCmd); 2] = [
            (
                "entity_insert_tag",
                "Attach the Rust component registered under Lua tag `tag`; an existing one is kept",
                |entity_id, tag| EntityCmd::InsertTag { entity_id, tag },
            ),
            (
                "entity_remove_tag",
                "Remove the Rust component registered under Lua tag `tag`; no-op without one",
                |entity_id, tag| EntityCmd::RemoveTag { entity_id, tag },
            ),
        ];
        for (name, desc, make) in fns {
            let full = format!("{prefix}{name}");
            engine.set(
                full.as_str(),
                self.lua
                    .create_function(move |lua, (entity_id, tag): (u64, LuaString)| {
                        let tag = resolve_lua_tag(lua, &tag.to_str()?)?;
                        let data = lua
                            .app_data_ref::<LuaAppData>()
                            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                        let queue = if collision {
                            &data.collision_entity_commands
                        } else {
                            &data.entity_commands
                        };
                        data.push(queue, make(entity_id, tag));
                        Ok(())
                    })?,
            )?;
            push_fn_meta(
                &self.lua,
                meta_fns,
                &full,
                desc,
                "entity",
                &[("entity_id", "integer"), ("tag", "string")],
                None,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::entity_builder::test_helpers::assert_runtime_error_in;
    use crate::resources::lua_runtime::EntityCmd;
    use crate::resources::lua_runtime::test_tags::runtime_with_test_tags;

    #[test]
    fn tag_functions_queue_resolved_tags_and_have_collision_twins() {
        let runtime = runtime_with_test_tags();
        runtime
            .lua()
            .load(
                "engine.entity_insert_tag(1, 'boid')\n\
                 engine.entity_remove_tag(1, 'leader')\n\
                 engine.collision_entity_insert_tag(1, 'fuel')",
            )
            .exec()
            .unwrap();

        let mut regular = Vec::new();
        runtime.drain_entity_commands_into(&mut regular);
        let regular: Vec<&EntityCmd> = regular.iter().map(|c| &c.cmd).collect();
        assert!(
            matches!(
                regular.as_slice(),
                [
                    EntityCmd::InsertTag { entity_id: 1, .. },
                    EntityCmd::RemoveTag { entity_id: 1, .. },
                ]
            ),
            "{regular:?}"
        );

        let mut collision = Vec::new();
        runtime.drain_collision_entity_commands_into(&mut collision);
        let collision: Vec<&EntityCmd> = collision.iter().map(|c| &c.cmd).collect();
        assert!(
            matches!(
                collision.as_slice(),
                [EntityCmd::InsertTag { entity_id: 1, .. }]
            ),
            "{collision:?}"
        );
    }

    #[test]
    fn unknown_tag_names_raise_at_the_call_and_queue_nothing() {
        let runtime = runtime_with_test_tags();
        for prefix in ["", "collision_"] {
            for verb in ["insert", "remove"] {
                assert_runtime_error_in(
                    &runtime,
                    &format!("engine.{prefix}entity_{verb}_tag(1, 'bod')"),
                    "Unknown Lua tag 'bod'",
                );
            }
        }

        let mut regular = Vec::new();
        runtime.drain_entity_commands_into(&mut regular);
        let mut collision = Vec::new();
        runtime.drain_collision_entity_commands_into(&mut collision);
        assert!(regular.is_empty(), "{regular:?}");
        assert!(collision.is_empty(), "{collision:?}");
    }
}
