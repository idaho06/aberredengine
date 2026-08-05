//! Lua-priority menu selection dispatch.
//!
//! Shadows `aberred_core::systems::menu::menu_selection_observer` with a
//! variant that checks the menu's `on_select_callback` (Lua) first, falling
//! back to its Rust fn-pointer callback, then `MenuActions` --
//! `aberred-core` cannot name `LuaRuntime`. Under `#[cfg(not(feature =
//! "lua"))]` the glob re-export below resolves to core's Rust-only variant
//! (and re-exports `menu_spawn_system`/`menu_despawn`/`menu_controller_observer`,
//! none of which differ by feature, regardless).

pub use aberred_core::systems::menu::*;

#[cfg(feature = "lua")]
use aberred_core::components::menu::{Menu, MenuActions};
#[cfg(feature = "lua")]
use aberred_core::events::menu::MenuSelectionEvent;
#[cfg(feature = "lua")]
use aberred_core::resources::gamestate::NextGameState;
#[cfg(feature = "lua")]
use aberred_core::resources::systemsstore::SystemsStore;
#[cfg(feature = "lua")]
use aberred_core::systems::GameCtx;
#[cfg(feature = "lua")]
use bevy_ecs::prelude::*;
#[cfg(feature = "lua")]
use log::{debug, error, warn};

/// Executes the action associated with a selected menu item.
///
/// Priority chain: Lua callback → Rust callback → `MenuActions`.
#[cfg(feature = "lua")]
pub fn menu_selection_observer(
    trigger: On<MenuSelectionEvent>,
    menus: Query<(&Menu, Option<&MenuActions>)>,
    mut next_game_state: ResMut<NextGameState>,
    systems_store: Res<SystemsStore>,
    mut ctx: GameCtx,
    lua_runtime: NonSend<crate::resources::lua_runtime::LuaRuntime>,
) {
    let event = trigger.event();
    debug!(
        "menu_selection_observer: Received MenuSelectionEvent for menu {:?}, item_id={}",
        event.menu, event.item_id
    );

    let Ok((menu, menu_actions_opt)) = menus.get(event.menu) else {
        warn!(
            "menu_selection_observer: Menu entity {:?} not found",
            event.menu
        );
        return;
    };

    // Priority 1: Lua callback
    if let Some(ref callback_name) = menu.on_select_callback {
        if lua_runtime.has_function(callback_name) {
            // Build context table
            let lua_ctx = lua_runtime.lua().create_table().unwrap();
            lua_ctx.set("menu_id", event.menu.to_bits()).unwrap();
            lua_ctx.set("item_id", event.item_id.clone()).unwrap();

            let item_index = menu
                .items
                .iter()
                .position(|item| item.id == event.item_id)
                .unwrap_or(0);
            lua_ctx.set("item_index", item_index).unwrap();

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
        let item_index = menu
            .items
            .iter()
            .position(|item| item.id == event.item_id)
            .unwrap_or(0);
        cb(event.menu, &event.item_id, item_index, &mut ctx);
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
