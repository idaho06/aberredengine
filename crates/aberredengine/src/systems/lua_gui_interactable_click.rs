//! Lua-priority GUI interactable click dispatch.
//!
//! Shadows [`aberred_core::systems::gui_interactable_click::gui_interactable_click_observer`]
//! with a variant that checks the entity's named Lua callback first, falling
//! back to its Rust fn-pointer callback -- mirrors
//! `aberred_core::systems::menu::menu_selection_observer`'s existing
//! priority chain. Re-exported by
//! [`crate::systems::gui_interactable_click`] under `#[cfg(feature = "lua")]`.

use aberred_core::events::gui_interactable::GuiInteractableClickEvent;
use aberred_core::systems::GameCtx;
use bevy_ecs::prelude::*;
use log::warn;

/// Reacts to `GuiInteractableClickEvent`; dispatches to the entity's named
/// Lua callback first, falling back to its Rust fn-pointer callback.
pub fn gui_interactable_click_observer(
    trigger: On<GuiInteractableClickEvent>,
    mut ctx: GameCtx,
    lua_runtime: bevy_ecs::system::NonSend<crate::resources::lua_runtime::LuaRuntime>,
) {
    let event = trigger.event();
    let Ok(interactable) = ctx.gui_interactables.get(event.entity) else {
        warn!(
            "gui_interactable_click_observer: entity {:?} not found",
            event.entity
        );
        return;
    };
    let on_click_callback = interactable.on_click_callback.clone();
    let on_rust_callback = interactable.on_rust_callback;

    // Priority 1: Lua callback
    if let Some(callback_name) = on_click_callback {
        if lua_runtime.has_function(&callback_name) {
            let lua_ctx = lua_runtime.lua().create_table().unwrap();
            lua_ctx.set("entity_id", event.entity.to_bits()).unwrap();
            if let Err(e) = lua_runtime.call_function::<_, ()>(&callback_name, lua_ctx) {
                log::error!(target: "lua", "Error in gui interactable callback '{}': {}", callback_name, e);
            }
        } else {
            warn!(target: "lua", "gui interactable callback '{}' not found", callback_name);
        }
        return;
    }

    // Priority 2: Rust callback
    if let Some(cb) = on_rust_callback {
        cb(event.entity, &mut ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::components::guiinteractable::GuiInteractable;
    use aberred_core::resources::appstate::AppState;
    use aberred_core::resources::camerafollowconfig::CameraFollowConfig;
    use aberred_core::resources::gameconfig::GameConfig;
    use aberred_core::resources::input_bindings::InputBindings;
    use aberred_core::resources::postprocessshader::PostProcessShader;
    use aberred_core::resources::sim_rng::SimRng;
    use aberred_core::resources::worldsignals::WorldSignals;
    use aberred_core::resources::worldtime::WorldTime;
    use bevy_ecs::message::Messages;
    use crate::resources::lua_runtime::LuaRuntime;

    fn setup_world() -> World {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.insert_resource(AppState::default());
        world.insert_resource(WorldTime::default());
        world.insert_resource(GameConfig::default());
        world.insert_resource(PostProcessShader::default());
        world.insert_resource(CameraFollowConfig::default());
        world.insert_resource(InputBindings::default());
        world.insert_resource(SimRng::from_seed(0));
        world.insert_resource(Messages::<aberred_core::protocol::audio::AudioCmd>::default());
        world.insert_non_send(LuaRuntime::new().expect("LuaRuntime::new"));
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
    fn lua_callback_takes_priority_over_rust_callback() {
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
}
