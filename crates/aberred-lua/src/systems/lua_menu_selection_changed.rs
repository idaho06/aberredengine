//! Lua menu selection-changed callbacks.
//!
//! [`lua_menu_selection_changed_observer`] calls a menu's [`LuaOnMenuSelectionChanged`]
//! callback (a Lua function name) when the highlight moves to another item. The engine
//! registers this observer only when a Lua script is set.

use crate::components::lua_on_menu_selection_changed::LuaOnMenuSelectionChanged;
use crate::systems::lua_commands::{LuaDispatch, dispatch_custom_and_drain};
use aberred_core::events::menu::MenuSelectionChanged;
use bevy_ecs::prelude::*;

/// Calls the menu's [`LuaOnMenuSelectionChanged`] callback with a ctx table of
/// `menu_id`, `item_id`, `item_index` and `previous_index`.
pub fn lua_menu_selection_changed_observer(
    trigger: On<MenuSelectionChanged>,
    menus: Query<&LuaOnMenuSelectionChanged>,
    mut p: LuaDispatch,
) {
    let event = trigger.event();
    let Ok(callback) = menus.get(event.entity) else {
        return;
    };
    dispatch_custom_and_drain(&mut p, &callback.callback, "Menu", |lua| {
        let ctx = lua.create_table()?;
        ctx.set("menu_id", event.entity.to_bits())?;
        ctx.set("item_id", event.item_id.as_str())?;
        ctx.set("item_index", event.index)?;
        ctx.set("previous_index", event.previous_index)?;
        Ok(ctx)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::components::menu::Menu;
    use aberred_core::math::Vec2;
    use aberred_core::resources::game_state::NextGameState;
    use aberred_core::resources::next_scene::NextScene;

    fn setup_world() -> World {
        let mut world = World::new();
        world.init_resource::<NextGameState>();
        world.init_resource::<NextScene>();
        init_dispatch_resources(&mut world);
        let lua = LuaRuntime::new().expect("LuaRuntime::new");
        lua.lua()
            .load(
                "function on_hint(ctx) \
                 hint = ctx.menu_id .. ':' .. ctx.item_id .. ':' .. ctx.item_index .. ':' .. ctx.previous_index \
                 end",
            )
            .exec()
            .expect("failed to load Lua function");
        world.insert_non_send(lua);
        world.add_observer(lua_menu_selection_changed_observer);
        world
    }

    fn two_item_menu() -> Menu {
        Menu::new(
            &[("play", "Play"), ("quit", "Quit")],
            Vec2::ZERO,
            "f",
            12.0,
            10.0,
            true,
        )
    }

    fn hint(world: &World) -> Option<String> {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get("hint")
            .unwrap()
    }

    fn move_to_quit(world: &mut World, menu: Entity) {
        world.trigger(MenuSelectionChanged {
            entity: menu,
            item_id: "quit".to_string(),
            index: 1,
            previous_index: 0,
        });
        world.flush();
    }

    #[test]
    fn callback_receives_menu_item_and_previous_index() {
        let mut world = setup_world();
        let menu = world
            .spawn((two_item_menu(), LuaOnMenuSelectionChanged::new("on_hint")))
            .id();
        move_to_quit(&mut world, menu);
        assert_eq!(hint(&world), Some(format!("{}:quit:1:0", menu.to_bits())));
    }

    #[test]
    fn a_menu_without_the_component_calls_nothing() {
        let mut world = setup_world();
        let menu = world.spawn(two_item_menu()).id();
        move_to_quit(&mut world, menu);
        assert_eq!(hint(&world), None);
    }
}
