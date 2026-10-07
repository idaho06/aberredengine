//! Lua GUI interactable click dispatch.
//!
//! [`lua_gui_interactable_click_observer`] is spawned only in games that run
//! a Lua script. It calls the clicked widget's [`LuaOnClick`] callback; Rust
//! code observes the same `GuiClicked` event directly.

use crate::components::lua_on_click::LuaOnClick;
use crate::systems::lua_commands::{LuaDispatch, dispatch_custom_and_drain};
use aberred_core::events::gui_interactable::GuiClicked;
use bevy_ecs::prelude::*;

/// Reacts to `GuiClicked` by calling the widget's [`LuaOnClick`] callback
/// with a `{ entity_id }` table. A widget without one is skipped.
pub fn lua_gui_interactable_click_observer(
    trigger: On<GuiClicked>,
    on_clicks: Query<&LuaOnClick>,
    mut p: LuaDispatch,
) {
    let entity = trigger.event().entity;
    let Ok(on_click) = on_clicks.get(entity) else {
        return;
    };
    dispatch_custom_and_drain(&mut p, &on_click.callback, "GUI interactable", |lua| {
        let ctx = lua.create_table()?;
        ctx.set("entity_id", entity.to_bits())?;
        Ok(ctx)
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::lua_runtime::LuaRuntime;
    use crate::systems::lua_commands::init_dispatch_resources;
    use aberred_core::components::guiinteractable::GuiInteractable;

    fn setup_world() -> World {
        let mut world = World::new();
        init_dispatch_resources(&mut world);
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
            .spawn((
                GuiInteractable::new(80.0, 24.0),
                LuaOnClick::new("on_gui_button_clicked"),
            ))
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
