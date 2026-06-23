//! GUI interactable click dispatch.
//!
//! [`gui_interactable_click_observer`] reacts to [`GuiInteractableClickEvent`]
//! (triggered by `gui_hit_test_system`) and resolves the callback chain on
//! the clicked entity's `GuiInteractable`: Lua name first, Rust fn-pointer
//! second — mirroring
//! [`menu_selection_observer`](crate::systems::menu::menu_selection_observer)'s
//! existing priority chain. Generalized from the former
//! `gui_button_click_observer` when `GuiInteractable` was extracted out of
//! `GuiButton` — this same observer now dispatches clicks for `GuiButton`,
//! `GuiImage`, and any future widget carrying `GuiInteractable`.

use bevy_ecs::prelude::*;
use log::warn;

use crate::components::guiinteractable::GuiInteractable;
use crate::events::gui_interactable::GuiInteractableClickEvent;
use crate::systems::GameCtx;

#[cfg(feature = "lua")]
pub fn gui_interactable_click_observer(
    trigger: On<GuiInteractableClickEvent>,
    interactables: Query<&GuiInteractable>,
    mut ctx: GameCtx,
    lua_runtime: bevy_ecs::system::NonSend<crate::resources::lua_runtime::LuaRuntime>,
) {
    let event = trigger.event();
    let Ok(interactable) = interactables.get(event.entity) else {
        warn!(
            "gui_interactable_click_observer: entity {:?} not found",
            event.entity
        );
        return;
    };

    // Priority 1: Lua callback
    if let Some(ref callback_name) = interactable.on_click_callback {
        if lua_runtime.has_function(callback_name) {
            let lua_ctx = lua_runtime.lua().create_table().unwrap();
            lua_ctx.set("entity_id", event.entity.to_bits()).unwrap();
            if let Err(e) = lua_runtime.call_function::<_, ()>(callback_name, lua_ctx) {
                log::error!(target: "lua", "Error in gui interactable callback '{}': {}", callback_name, e);
            }
        } else {
            warn!(target: "lua", "gui interactable callback '{}' not found", callback_name);
        }
        return;
    }

    // Priority 2: Rust callback
    if let Some(cb) = interactable.on_rust_callback {
        cb(event.entity, &mut ctx);
    }
}

#[cfg(not(feature = "lua"))]
pub fn gui_interactable_click_observer(
    trigger: On<GuiInteractableClickEvent>,
    interactables: Query<&GuiInteractable>,
    mut ctx: GameCtx,
) {
    let event = trigger.event();
    let Ok(interactable) = interactables.get(event.entity) else {
        warn!(
            "gui_interactable_click_observer: entity {:?} not found",
            event.entity
        );
        return;
    };

    if let Some(cb) = interactable.on_rust_callback {
        cb(event.entity, &mut ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::guiimage::GuiImage;
    use crate::resources::appstate::AppState;
    use crate::resources::camerafollowconfig::CameraFollowConfig;
    use crate::resources::gameconfig::GameConfig;
    use crate::resources::input_bindings::InputBindings;
    use crate::resources::postprocessshader::PostProcessShader;
    use crate::resources::texturestore::TextureStore;
    use crate::resources::worldsignals::WorldSignals;
    use crate::resources::worldtime::WorldTime;
    use bevy_ecs::message::Messages;

    fn setup_world() -> World {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(AppState::default());
        world.insert_resource(WorldTime::default());
        world.insert_resource(TextureStore::new());
        world.insert_resource(GameConfig::default());
        world.insert_resource(PostProcessShader::default());
        world.insert_resource(CameraFollowConfig::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(Messages::<crate::events::audio::AudioCmd>::default());
        #[cfg(feature = "lua")]
        world.insert_non_send(
            crate::resources::lua_runtime::LuaRuntime::new().expect("LuaRuntime::new"),
        );
        world
    }

    fn tick(world: &mut World) {
        world.spawn(Observer::new(gui_interactable_click_observer));
        world.flush();
    }

    fn dummy_callback(entity: Entity, ctx: &mut GameCtx) {
        ctx.world_signals.set_flag("rust_callback_fired");
        let _ = entity;
    }

    #[test]
    fn rust_callback_fires_when_no_lua_callback_set() {
        let mut world = setup_world();
        let button = world
            .spawn(GuiInteractable::rust(80.0, 24.0, dummy_callback))
            .id();

        tick(&mut world);
        world.trigger(GuiInteractableClickEvent { entity: button });
        world.flush();

        assert!(
            world
                .resource::<WorldSignals>()
                .has_flag("rust_callback_fired")
        );
    }

    #[cfg(feature = "lua")]
    #[test]
    fn lua_callback_takes_priority_over_rust_callback() {
        use crate::resources::lua_runtime::LuaRuntime;

        let mut world = setup_world();
        {
            let lua_rt = world.non_send::<LuaRuntime>();
            lua_rt
                .lua()
                .load("function on_gui_button_clicked() end")
                .exec()
                .expect("failed to load Lua function");
        }

        let button = world
            .spawn(
                GuiInteractable::rust(80.0, 24.0, dummy_callback)
                    .with_on_click_callback("on_gui_button_clicked"),
            )
            .id();

        tick(&mut world);
        world.trigger(GuiInteractableClickEvent { entity: button });
        world.flush();

        assert!(
            !world
                .resource::<WorldSignals>()
                .has_flag("rust_callback_fired"),
            "Rust callback should be skipped when a Lua callback is set"
        );
    }

    #[test]
    fn missing_button_entity_does_not_panic() {
        let mut world = setup_world();
        let bogus = world.spawn_empty().id();
        world.despawn(bogus);

        tick(&mut world);
        world.trigger(GuiInteractableClickEvent { entity: bogus });
        world.flush();
    }

    #[test]
    fn gui_image_click_dispatches_through_same_observer() {
        let mut world = setup_world();
        let image = world
            .spawn((
                GuiImage::new(40.0, 40.0, "item_sword"),
                GuiInteractable::rust(40.0, 40.0, dummy_callback),
            ))
            .id();

        tick(&mut world);
        world.trigger(GuiInteractableClickEvent { entity: image });
        world.flush();

        assert!(
            world
                .resource::<WorldSignals>()
                .has_flag("rust_callback_fired")
        );
    }
}
