//! Rust components a Lua script attaches by name.
//!
//! A hybrid game registers a component type under a name (`EngineBuilder::lua_tag`); Lua then
//! attaches it with `:with_tag(name)` on a spawn or clone builder, or with
//! `engine.entity_insert_tag(id, name)` / `engine.entity_remove_tag(id, name)` at runtime.
//! Names resolve to a [`LuaTag`] when Lua calls, so queued commands carry no names.

use super::runtime::{LuaAppData, LuaRuntime};
use bevy_ecs::prelude::Component;
use bevy_ecs::world::EntityWorldMut;
use mlua::prelude::*;

/// How to attach and detach one Rust component registered as a Lua tag.
#[derive(Debug, Clone, Copy)]
pub struct LuaTag {
    insert: fn(&mut EntityWorldMut<'_>),
    remove: fn(&mut EntityWorldMut<'_>),
}

impl LuaTag {
    /// The tag for component `T`: inserts `T::default()` unless the entity already has a `T`,
    /// and removes `T`.
    pub fn of<T: Component + Default + Clone>() -> LuaTag {
        LuaTag {
            insert: insert_tag::<T>,
            remove: remove_tag::<T>,
        }
    }

    /// Inserts the tag's component, keeping one the entity already has.
    pub(crate) fn insert(self, entity: &mut EntityWorldMut<'_>) {
        (self.insert)(entity);
    }

    /// Removes the tag's component; a no-op without one.
    pub(crate) fn remove(self, entity: &mut EntityWorldMut<'_>) {
        (self.remove)(entity);
    }
}

fn insert_tag<T: Component + Default>(entity: &mut EntityWorldMut<'_>) {
    entity.insert_if_new(T::default());
}

fn remove_tag<T: Component>(entity: &mut EntityWorldMut<'_>) {
    entity.remove::<T>();
}

impl LuaRuntime {
    /// Lets Lua attach `tag` by `name`. Registering a name again replaces its tag.
    pub fn register_lua_tag(&self, name: impl Into<String>, tag: LuaTag) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            data.lua_tags.borrow_mut().insert(name.into(), tag);
        }
    }
}

/// Resolves a Lua tag name; an unregistered name is a Lua runtime error listing the
/// registered names.
pub(super) fn resolve_lua_tag(lua: &Lua, name: &str) -> LuaResult<LuaTag> {
    let data = lua
        .app_data_ref::<LuaAppData>()
        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
    let tags = data.lua_tags.borrow();
    tags.get(name).copied().ok_or_else(|| {
        let mut names: Vec<&str> = tags.keys().map(String::as_str).collect();
        names.sort_unstable();
        let list = if names.is_empty() {
            "none".to_string()
        } else {
            names.join(", ")
        };
        LuaError::runtime(format!("Unknown Lua tag '{name}' (registered: {list})"))
    })
}

#[cfg(test)]
pub(crate) mod test_tags {
    use super::LuaTag;
    use crate::resources::lua_runtime::LuaRuntime;
    use bevy_ecs::prelude::Component;

    #[derive(Component, Default, Clone, Debug, PartialEq)]
    pub(crate) struct Boid;
    #[derive(Component, Default, Clone, Debug, PartialEq)]
    pub(crate) struct Leader;
    #[derive(Component, Default, Clone, Debug, PartialEq)]
    pub(crate) struct Fuel(pub u32);

    /// A runtime with "boid", "leader" and "fuel" registered.
    pub(crate) fn runtime_with_test_tags() -> LuaRuntime {
        let runtime = LuaRuntime::new().unwrap();
        runtime.register_lua_tag("boid", LuaTag::of::<Boid>());
        runtime.register_lua_tag("leader", LuaTag::of::<Leader>());
        runtime.register_lua_tag("fuel", LuaTag::of::<Fuel>());
        runtime
    }
}

#[cfg(test)]
mod tests {
    use super::super::entity_builder::test_helpers::{
        assert_runtime_error, assert_runtime_error_in,
    };
    use super::test_tags::runtime_with_test_tags;

    #[test]
    fn unknown_tag_names_raise_and_list_the_registered_tags() {
        assert_runtime_error(
            "engine.spawn():with_tag('boid')",
            "Unknown Lua tag 'boid' (registered: none)",
        );
        assert_runtime_error_in(
            &runtime_with_test_tags(),
            "engine.clone('tpl'):with_tag('bod')",
            "Unknown Lua tag 'bod' (registered: boid, fuel, leader)",
        );
    }
}
