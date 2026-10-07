//! Lua menu selection callbacks.
//!
//! [`lua_menu_selection_observer`] calls a menu's [`LuaOnMenuSelect`] callback
//! (a Lua function name) when one of its items is confirmed. A Lua menu with a
//! callback is spawned without `MenuActions`. The engine registers this
//! observer only when a Lua script is set.

use crate::components::lua_on_menu_select::LuaOnMenuSelect;
use crate::systems::lua_commands::{LuaDispatch, dispatch_custom_and_drain};
use aberred_core::events::menu::MenuSelected;
use bevy_ecs::prelude::*;

/// Calls the selected menu's [`LuaOnMenuSelect`] callback with a ctx table of
/// `menu_id`, `item_id` and `item_index`.
pub fn lua_menu_selection_observer(
    trigger: On<MenuSelected>,
    menus: Query<&LuaOnMenuSelect>,
    mut p: LuaDispatch,
) {
    let event = trigger.event();
    let Ok(on_select) = menus.get(event.entity) else {
        return;
    };
    dispatch_custom_and_drain(&mut p, &on_select.callback, "Menu", |lua| {
        let ctx = lua.create_table()?;
        ctx.set("menu_id", event.entity.to_bits())?;
        ctx.set("item_id", event.item_id.as_str())?;
        ctx.set("item_index", event.index)?;
        Ok(ctx)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::components::menu::{Menu, MenuAction, MenuActions};
    use aberred_core::math::Vec2;
    use aberred_core::resources::gamestate::NextGameState;
    use aberred_core::resources::signal_keys as sk;
    use aberred_core::resources::worldsignals::WorldSignals;
    use aberred_core::systems::menu::menu_selection_observer;

    /// Both menu observers, as the engine registers them in a Lua game.
    fn setup_world() -> World {
        let mut world = World::new();
        world.init_resource::<NextGameState>();
        init_dispatch_resources(&mut world);
        let lua = LuaRuntime::new().expect("LuaRuntime::new");
        lua.lua()
            .load("function on_menu_select(ctx) menu_selected_item = ctx.item_id .. ':' .. ctx.item_index end")
            .exec()
            .expect("failed to load Lua function");
        world.insert_non_send(lua);
        world.add_observer(menu_selection_observer);
        world.add_observer(lua_menu_selection_observer);
        world
    }

    /// A one-item menu, spawned as Lua's `spawn_cmd` does: with a callback it
    /// gets [`LuaOnMenuSelect`] and no `MenuActions`.
    fn spawn_play_menu(world: &mut World, lua_callback: bool) -> Entity {
        let menu = Menu::new(&[("play", "Play")], Vec2::ZERO, "f", 12.0, 10.0, true);
        if lua_callback {
            world
                .spawn((menu, LuaOnMenuSelect::new("on_menu_select")))
                .id()
        } else {
            let actions = MenuActions::new().with("play", MenuAction::SetScene("level01".into()));
            world.spawn((menu, actions)).id()
        }
    }

    fn select_play(world: &mut World, menu: Entity) {
        world.trigger(MenuSelected {
            entity: menu,
            item_id: "play".to_string(),
            index: 0,
        });
        world.flush();
    }

    fn lua_selected_item(world: &World) -> Option<String> {
        world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get("menu_selected_item")
            .unwrap()
    }

    fn record_observed(_trigger: On<MenuSelected>, mut ws: ResMut<WorldSignals>) {
        ws.set_flag("observed");
    }

    #[test]
    fn lua_callback_runs_and_observers_still_run() {
        let mut world = setup_world();
        let menu = spawn_play_menu(&mut world, true);
        world.entity_mut(menu).observe(record_observed);
        select_play(&mut world, menu);
        assert_eq!(lua_selected_item(&world).as_deref(), Some("play:0"));
        let ws = world.resource::<WorldSignals>();
        assert_eq!(ws.get_string(sk::SCENE), None, "no MenuActions ran");
        assert!(ws.has_flag("observed"));
    }

    #[test]
    fn without_a_lua_callback_menu_actions_run() {
        let mut world = setup_world();
        let menu = spawn_play_menu(&mut world, false);
        select_play(&mut world, menu);
        assert_eq!(lua_selected_item(&world), None);
        assert_eq!(
            world.resource::<WorldSignals>().get_string(sk::SCENE),
            Some("level01")
        );
    }
}
