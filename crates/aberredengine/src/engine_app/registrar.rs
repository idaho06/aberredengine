use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;

use super::logic_world::register_persistent_system;
use super::schedule::SimSet;
use aberred_core::resources::scenemanager::SceneManager;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::systems::gamestate::state_is_playing;

/// Closure that registers a system into the world and inserts its ID into
/// [`SystemsStore`]. Deferred until `run()` when the [`World`] exists.
/// `Send` because these are carried into the logic thread via `LogicInit`.
pub(crate) type HookRegistrar = Box<dyn FnOnce(&mut World, &mut SystemsStore) + Send>;

/// Closure that adds a game-update system to the [`Schedule`].
/// Deferred until `run()` when the schedule is being built.
/// `Send`: see [`HookRegistrar`].
pub(crate) type UpdateRegistrar = Box<dyn FnOnce(&mut Schedule) + Send>;

/// Closure that spawns an observer entity into the [`World`].
/// Deferred until `run()` when the world exists.
/// `Send`: see [`HookRegistrar`].
pub(crate) type ObserverRegistrar = Box<dyn FnOnce(&mut World) + Send>;

/// Build a [`HookRegistrar`] that registers `system` as a persistent system
/// under `name` when run. Collapses the
/// `Box::new(|world, store| register_persistent_system(world, store, name, system))`
/// closure repeated across `on_setup`/`on_enter_play`/`on_switch_scene` and
/// `with_lua`'s hook installations into one call site.
pub(crate) fn hook_registrar<M>(
    name: &'static str,
    system: impl IntoSystem<(), (), M> + Send + 'static,
) -> HookRegistrar {
    Box::new(move |world, store| register_persistent_system(world, store, name, system))
}

/// Build the [`UpdateRegistrar`] behind `add_system`: `system` runs in
/// [`SimSet::ScriptUpdate`] while the game is `Playing`.
pub(crate) fn system_registrar<M>(
    system: impl IntoSystem<(), (), M> + Send + 'static,
) -> UpdateRegistrar {
    Box::new(move |schedule| {
        schedule.add_systems(system.run_if(state_is_playing).in_set(SimSet::ScriptUpdate));
    })
}

/// Build the [`UpdateRegistrar`] behind `add_system_if`: as
/// [`system_registrar`], and only while `condition` holds.
pub(crate) fn conditional_system_registrar<M, MC>(
    system: impl IntoSystem<(), (), M> + Send + 'static,
    condition: impl SystemCondition<MC> + Send + 'static,
) -> UpdateRegistrar {
    Box::new(move |schedule| {
        schedule.add_systems(
            system
                .run_if(state_is_playing)
                .run_if(condition)
                .in_set(SimSet::ScriptUpdate),
        );
    })
}

/// Build the [`ObserverRegistrar`] behind `on_scene_enter`/`on_scene_exit`:
/// attaches `observer` to `scene`'s scene entity, so it fires only for that
/// scene. The scene entity is `Persistent`, so its observer survives switches.
///
/// Runs after the `SceneManager` and its scene entities exist; an unregistered
/// `scene` is rejected earlier by `validate_builder`.
pub(crate) fn scene_observer_registrar<E: EntityEvent, B: Bundle, M>(
    scene: &'static str,
    observer: impl IntoObserverSystem<E, B, M>,
) -> ObserverRegistrar {
    Box::new(move |world| {
        let entity = world
            .get_resource::<SceneManager>()
            .and_then(|scenes| scenes.scene_entity(scene))
            .unwrap_or_else(|| panic!("scene '{scene}' is not registered with .add_scene()"));
        world.entity_mut(entity).observe(observer);
    })
}
