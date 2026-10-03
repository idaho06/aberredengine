use bevy_ecs::prelude::*;
use bevy_ecs::system::IntoObserverSystem;

use super::logic_world::register_persistent_system;
use super::schedule::SimSet;
use aberred_core::components::persistent::Persistent;
use aberred_core::resources::scenemanager::SceneManager;
use aberred_core::resources::systemsstore::SystemsStore;
use aberred_core::systems::gamestate::{state_is_playing, state_runs_scenes};
use aberred_core::systems::scene_dispatch::in_scene;

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
/// under `name` when run (`on_setup` on both builders).
pub(crate) fn hook_registrar<M>(
    name: &'static str,
    system: impl IntoSystem<(), (), M> + Send + 'static,
) -> HookRegistrar {
    Box::new(move |world, store| register_persistent_system(world, store, name, system))
}

/// `system` in [`SimSet::ScriptUpdate`], run while `gate` holds. The named
/// registrars below choose the gate, so both builders share one rule.
fn system_registrar<M, MG>(
    system: impl IntoSystem<(), (), M> + Send + 'static,
    gate: impl SystemCondition<MG> + Send + 'static,
) -> UpdateRegistrar {
    Box::new(move |schedule| {
        schedule.add_systems(system.run_if(gate).in_set(SimSet::ScriptUpdate));
    })
}

/// `add_system`: runs while `Playing`.
pub(crate) fn playing_system<M>(
    system: impl IntoSystem<(), (), M> + Send + 'static,
) -> UpdateRegistrar {
    system_registrar(system, state_is_playing)
}

/// `add_system_if`: runs while `Playing` and `condition` holds.
pub(crate) fn playing_system_if<M, MC>(
    system: impl IntoSystem<(), (), M> + Send + 'static,
    condition: impl SystemCondition<MC> + Send + 'static,
) -> UpdateRegistrar {
    system_registrar(system, state_is_playing.and_then(condition))
}

/// `add_scene_system`: runs while `scene` is active, in `Setup` (a loading
/// scene) as well as `Playing`.
pub(crate) fn scene_system<M>(
    scene: &'static str,
    system: impl IntoSystem<(), (), M> + Send + 'static,
) -> UpdateRegistrar {
    system_registrar(system, state_runs_scenes.and_then(in_scene(scene)))
}

/// Build the [`ObserverRegistrar`] behind `add_observer`: a global observer,
/// spawned [`Persistent`] so scene switches keep it.
pub(crate) fn observer_registrar<E: Event, B: Bundle, M>(
    observer: impl IntoObserverSystem<E, B, M>,
) -> ObserverRegistrar {
    Box::new(move |world| {
        world.spawn((Observer::new(observer), Persistent));
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
