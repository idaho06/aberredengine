use bevy_ecs::prelude::*;

use super::logic_world::register_persistent_system;
use aberred_core::resources::systemsstore::SystemsStore;

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
