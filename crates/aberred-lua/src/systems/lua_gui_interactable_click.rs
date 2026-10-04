//! Lua GUI interactable click dispatch.
//!
//! [`lua_gui_interactable_click_observer`] is spawned only in games that run
//! a Lua script. It calls the clicked widget's named Lua callback; Rust code
//! observes the same `GuiClicked` event directly.

use crate::resources::lua_runtime::LuaRuntime;
use aberred_core::components::guiinteractable::GuiInteractable;
use aberred_core::events::gui_interactable::GuiClicked;
use bevy_ecs::prelude::*;
use log::warn;

/// Reacts to `GuiClicked` by calling the widget's Lua callback
/// (`GuiInteractable::on_click_callback`) with a `{ entity_id }` table.
pub fn lua_gui_interactable_click_observer(
    trigger: On<GuiClicked>,
    interactables: Query<&GuiInteractable>,
    lua_runtime: NonSend<LuaRuntime>,
) {
    let entity = trigger.event().entity;
    let Ok(interactable) = interactables.get(entity) else {
        warn!("lua_gui_interactable_click_observer: entity {entity:?} not found");
        return;
    };
    let Some(callback_name) = &interactable.on_click_callback else {
        return;
    };
    lua_runtime.call_named(callback_name, "GUI interactable", |f| {
        let lua_ctx = lua_runtime.lua().create_table()?;
        lua_ctx.set("entity_id", entity.to_bits())?;
        f.call::<()>(lua_ctx)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_world() -> World {
        let mut world = World::new();
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
        world.spawn(Observer::new(lua_gui_interactable_click_observer));
        world.flush();
        world
    }

    #[test]
    fn click_calls_the_lua_callback_with_the_entity_id() {
        let mut world = setup_world();
        world
            .non_send::<LuaRuntime>()
            .lua()
            .load("function on_gui_button_clicked(ctx) clicked_entity = ctx.entity_id end")
            .exec()
            .expect("failed to load Lua function");

        let button = world
            .spawn(GuiInteractable::new(80.0, 24.0).with_on_click_callback("on_gui_button_clicked"))
            .id();
        world.trigger(GuiClicked { entity: button });
        world.flush();

        let clicked: Option<u64> = world
            .non_send::<LuaRuntime>()
            .lua()
            .globals()
            .get("clicked_entity")
            .unwrap();
        assert_eq!(clicked, Some(button.to_bits()));
    }
}
