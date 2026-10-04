//! Lua-priority menu selection dispatch.
//!
//! Shadows `aberred_core::systems::menu::menu_selection_observer` with a
//! variant that checks the menu's `on_select_callback` (Lua) first, falling
//! back to its Rust fn-pointer callback, then `MenuActions` --
//! `aberred-core` cannot name `LuaRuntime`. Re-exported by
//! the facade's `systems::menu` under `#[cfg(feature = "lua")]`.

use aberred_core::components::menu::{Menu, MenuActions};
use aberred_core::events::menu::MenuSelected;
use aberred_core::resources::gamestate::NextGameState;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::systems::GameCtx;
use bevy_ecs::prelude::*;
use log::{debug, error, warn};

/// Executes the action associated with a selected menu item.
///
/// Priority chain: Lua callback → Rust callback → `MenuActions`.
pub fn menu_selection_observer(
    trigger: On<MenuSelected>,
    menus: Query<(&Menu, Option<&MenuActions>)>,
    mut next_game_state: ResMut<NextGameState>,
    systems_store: Res<SystemsStore>,
    mut ctx: GameCtx,
    lua_runtime: NonSend<crate::resources::lua_runtime::LuaRuntime>,
) {
    let event = trigger.event();
    debug!(
        "menu_selection_observer: Received MenuSelected for menu {:?}, item_id={}",
        event.entity, event.item_id
    );

    let Ok((menu, menu_actions_opt)) = menus.get(event.entity) else {
        warn!(
            "menu_selection_observer: Menu entity {:?} not found",
            event.entity
        );
        return;
    };

    // Priority 1: Lua callback
    if let Some(ref callback_name) = menu.on_select_callback {
        if lua_runtime.has_function(callback_name) {
            // Build context table
            let lua_ctx = lua_runtime.lua().create_table().unwrap();
            lua_ctx.set("menu_id", event.entity.to_bits()).unwrap();
            lua_ctx.set("item_id", event.item_id.as_str()).unwrap();
            lua_ctx.set("item_index", event.index).unwrap();

            if let Err(e) = lua_runtime.call_function::<_, ()>(callback_name, lua_ctx) {
                error!(target: "lua", "Error in menu callback '{}': {}", callback_name, e);
            }
        } else {
            warn!(target: "lua", "menu callback '{}' not found", callback_name);
        }
        return;
    }

    // Priority 2: Rust callback
    if let Some(cb) = menu.on_rust_callback {
        cb(event.entity, &event.item_id, event.index, &mut ctx);
        return;
    }

    // Priority 3: MenuActions
    aberred_core::systems::menu::dispatch_menu_action(
        menu_actions_opt,
        event,
        &mut ctx,
        &mut next_game_state,
        &systems_store,
    );
}
