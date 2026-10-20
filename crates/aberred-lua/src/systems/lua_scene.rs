//! Lua scene callbacks.
//!
//! Core `scene_switch_system` switches scenes; these observers add the Lua
//! side. [`lua_scene_exited_observer`] drops the commands queued for the scene
//! being left, before its entities are despawned. [`lua_scene_entered_observer`]
//! calls `on_switch_scene` once the old scene is gone. The engine registers
//! them only when a Lua script is set. Their order relative to other global
//! observers of the same event is undefined.

use crate::lua_plugin::DrainCtx;
use crate::resources::lua_runtime::LuaRuntime;
use aberred_core::events::scene::{SceneEntered, SceneExited};
use bevy_ecs::prelude::*;
use log::error;

/// Clears the `clear`-policy Lua command queues, which may reference entities
/// of the scene being left; `preserve` queues (maps, assets, GUI themes) keep
/// their commands.
pub fn lua_scene_exited_observer(_: On<SceneExited>, lua_runtime: NonSend<LuaRuntime>) {
    lua_runtime.clear_all_commands();
}

/// Prepares the Lua runtime for the entered scene and calls
/// `on_switch_scene(name)`: drops cached function handles (the scene's
/// callbacks are re-injected), refreshes the tracked-group and signal caches so
/// the callback sees the post-teardown state, drains the commands it queued,
/// and refreshes the config cache.
pub fn lua_scene_entered_observer(trigger: On<SceneEntered>, mut ctx: DrainCtx) {
    let lua_runtime = &ctx.scripting.lua_runtime;
    lua_runtime.clear_function_cache();
    lua_runtime.update_tracked_groups_cache(&ctx.tracked_groups.groups);
    lua_runtime.sync_signals(&mut ctx.scene_state.world_signals);

    let name = &*trigger.event().name;
    if lua_runtime.has_function("on_switch_scene")
        && let Err(e) = lua_runtime.call_function::<_, ()>("on_switch_scene", name)
    {
        error!("Error calling on_switch_scene: {}", e);
    }

    ctx.drain();
    ctx.scripting
        .lua_runtime
        .update_gameconfig_cache(&ctx.scene_state.config);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua_plugin::{lua_exec, lua_global, new_drain_test_world};
    use aberred_core::resources::worldsignals::WorldSignals;
    use std::sync::Arc;

    /// A Lua test world with both scene observers.
    fn scene_observer_world() -> World {
        let mut world = new_drain_test_world();
        world.add_observer(lua_scene_exited_observer);
        world.add_observer(lua_scene_entered_observer);
        world
    }

    /// Triggers [`SceneEntered`] for `name`, then applies the observer's
    /// commands, as the next sync point does in the engine.
    fn enter(world: &mut World, name: &str) {
        let scene = world.spawn_empty().id();
        world.trigger(SceneEntered {
            scene,
            name: Arc::from(name),
            previous: None,
        });
        world.flush();
    }

    /// Leaving a scene clears the `clear`-policy queues and keeps the
    /// `preserve` ones (maps, assets, GUI themes).
    #[test]
    fn exited_clears_scene_queues_and_keeps_preserved_ones() {
        let mut world = scene_observer_world();
        lua_exec(
            &world,
            "engine.load_map('maps/dummy.json')\n\
             engine.load_texture('boss', 'assets/boss.png')\n\
             engine.set_gui_theme_font('default', 'f', 12, 255, 255, 255, 255)\n\
             engine.set_flag('stale_flag')",
        );
        let scene = world.spawn_empty().id();

        world.trigger(SceneExited {
            scene,
            name: Arc::from("menu"),
            next: Arc::from("level"),
        });

        let lua_runtime = world.non_send::<LuaRuntime>();
        let (mut maps, mut assets, mut themes, mut signals) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        lua_runtime.drain_map_commands_into(&mut maps);
        lua_runtime.drain_asset_commands_into(&mut assets);
        lua_runtime.drain_gui_theme_commands_into(&mut themes);
        lua_runtime.drain_signal_commands_into(&mut signals);
        assert_eq!((maps.len(), assets.len(), themes.len()), (1, 1, 1));
        assert!(
            signals.is_empty(),
            "scene-scoped signal_commands are cleared"
        );
    }

    /// `on_switch_scene` gets the entered scene's name and sees the current
    /// signals, not a stale cached snapshot.
    #[test]
    fn entered_refreshes_the_signal_cache_before_on_switch_scene() {
        let mut world = scene_observer_world();
        let mut stale = WorldSignals::default();
        stale.set_entity("player", world.spawn_empty().id());
        world
            .non_send::<LuaRuntime>()
            .update_signal_cache(stale.snapshot());
        lua_exec(
            &world,
            "function on_switch_scene(scene)\n\
                 switched_to = scene\n\
                 player_seen = engine.get_entity('player')\n\
             end",
        );

        enter(&mut world, "level");

        assert_eq!(lua_global::<String>(&world, "switched_to"), "level");
        assert_eq!(lua_global::<Option<u64>>(&world, "player_seen"), None);
    }

    /// A spawn queued in `on_switch_scene` is drained by the observer.
    #[test]
    fn entered_drains_commands_queued_by_on_switch_scene() {
        let mut world = scene_observer_world();
        lua_exec(
            &world,
            "function on_switch_scene(scene) engine.spawn():register_as('hero'):build() end",
        );

        enter(&mut world, "level");

        let hero = world.resource::<WorldSignals>().get_entity("hero");
        assert!(hero.is_some_and(|hero| world.get_entity(hero).is_ok()));
    }

    /// Entering a scene drops cached function handles, so the scene's
    /// re-injected callbacks resolve fresh.
    #[test]
    fn entered_clears_the_function_cache() {
        let mut world = scene_observer_world();
        lua_exec(&world, "function on_update_level() return 1 end");
        world
            .non_send::<LuaRuntime>()
            .get_function_cached("on_update_level")
            .unwrap();
        lua_exec(&world, "function on_update_level() return 2 end");

        enter(&mut world, "level");

        let f = world
            .non_send::<LuaRuntime>()
            .get_function_cached("on_update_level")
            .unwrap()
            .expect("defined");
        assert_eq!(f.call::<i64>(()).unwrap(), 2);
    }
}
