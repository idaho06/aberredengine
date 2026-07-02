//! Generic typed state store passed to [`GuiCallback`](crate::systems::scene_dispatch::GuiCallback).
//!
//! Stores one value per Rust type, keyed by [`TypeId`]. Access is type-safe at call sites:
//! [`insert`](AppState::insert)`(value: T)` and [`get`](AppState::get)`::<T>()` infer the
//! key from `T` — no string constants needed.
//!
//! # One slot per type
//!
//! Use newtypes when you need two values of the same underlying type:
//! `struct BeforeSnapshot(ComponentSnapshot)` vs `struct AfterSnapshot(ComponentSnapshot)`.
//!
//! # `T: Clone` required (Phase 5d)
//!
//! [`insert`](AppState::insert) requires `T: Clone` — `AppState` itself is `Clone` (see
//! below), which lets `DrawableSnapshot` carry a cloned copy for render-side scene
//! callbacks. A type that wants shared interior mutability across a clone (the common case
//! for editor-style caches) should be wrapped as `Arc<Mutex<T>>` (or `Arc<RwLock<T>>`) rather
//! than stored as a bare `Mutex<T>`/`RwLock<T>` — those are never `Clone` regardless of `T`.
//! Cloning `Arc<Mutex<T>>` is a cheap pointer clone and preserves aliasing: mutating through
//! the `Mutex` is visible on every clone, which is exactly the semantics that pattern is
//! for.
//!
//! # Auto-inserted
//!
//! [`AppState::default()`] is inserted by the engine at startup. Games and editors do not
//! need to insert it manually.
//!
//! # Example
//!
//! ```rust,ignore
//! // In an ECS observer — write typed state:
//! app_state.insert(MySnapshot { value: 42 });
//!
//! // In a GuiCallback — read typed state:
//! if let Some(snap) = app_state.get::<MySnapshot>() {
//!     ui.text(format!("value: {}", snap.value));
//! }
//! ```

use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;
use std::any::{Any, TypeId};

/// One stored slot: the boxed value (via std's blessed `Box<dyn Any + Send + Sync>`
/// downcasting, so `get`/`get_mut`/`remove` stay exactly as simple as before), plus a
/// monomorphized clone function captured at [`insert`](AppState::insert) time — this is what
/// makes [`AppState`] itself `Clone` without needing a custom downcastable trait object (which
/// runs into an `Any`/lifetime-elision dead end: a hand-rolled `dyn AppStateValue: Any` cannot
/// reuse std's `downcast_ref`, which is only inherently implemented for `dyn Any`/
/// `dyn Any + Send`/`dyn Any + Send + Sync`).
struct Entry {
    value: Box<dyn Any + Send + Sync>,
    clone_fn: fn(&(dyn Any + Send + Sync)) -> Box<dyn Any + Send + Sync>,
}

fn clone_entry_value<T: Any + Send + Sync + Clone>(
    value: &(dyn Any + Send + Sync),
) -> Box<dyn Any + Send + Sync> {
    Box::new(
        value
            .downcast_ref::<T>()
            .expect("Entry::clone_fn always matches the type it was created for")
            .clone(),
    )
}

/// Generic typed state store for ECS-to-GUI communication.
///
/// See the [module documentation](self) for usage guidelines.
///
/// # Part of `DrawableSnapshot` (Phase 5d)
///
/// The Option B render/logic thread split
/// (`docs/render-simulation-separation-brainstorm.md`) snapshots render-relevant state into
/// `DrawableSnapshot`, which carries a cloned `AppState` (Phase 5d). Each stored [`Entry`]
/// carries its own clone function captured at insert time, which is what makes `AppState`
/// itself `Clone` and imposes the breaking `T: Clone` bound on [`insert`](AppState::insert).
/// A `generation` counter, bumped by [`insert`](AppState::insert), [`get_mut`](AppState::get_mut),
/// and a successful [`remove`](AppState::remove), lets `build_drawable_snapshot` skip the
/// clone on frames where nothing changed — `get_mut` bumps even if the caller doesn't
/// actually mutate through the returned reference, which is conservative over-cloning, never
/// staleness.
#[derive(Resource, Default)]
pub struct AppState {
    map: FxHashMap<TypeId, Entry>,
    generation: u64,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("len", &self.map.len())
            .field("generation", &self.generation)
            .finish()
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        AppState {
            map: self
                .map
                .iter()
                .map(|(type_id, entry)| {
                    (
                        *type_id,
                        Entry {
                            value: (entry.clone_fn)(&*entry.value),
                            clone_fn: entry.clone_fn,
                        },
                    )
                })
                .collect(),
            generation: self.generation,
        }
    }
}

impl AppState {
    /// Store a value. Replaces any previous value of type `T`. Bumps [`generation`](Self::generation).
    pub fn insert<T: Any + Send + Sync + Clone>(&mut self, value: T) {
        self.map.insert(
            TypeId::of::<T>(),
            Entry {
                value: Box::new(value),
                clone_fn: clone_entry_value::<T>,
            },
        );
        self.generation += 1;
    }

    /// Return a shared reference to the stored value of type `T`, or `None`.
    pub fn get<T: Any + 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.value.downcast_ref::<T>()
    }

    /// Return an exclusive reference to the stored value of type `T`, or `None`.
    ///
    /// Bumps [`generation`](Self::generation) unconditionally, even if the caller ends up not
    /// mutating the returned reference or the type isn't stored at all.
    pub fn get_mut<T: Any + 'static>(&mut self) -> Option<&mut T> {
        self.generation += 1;
        self.map
            .get_mut(&TypeId::of::<T>())?
            .value
            .downcast_mut::<T>()
    }

    /// Remove and return the stored value of type `T`, or `None`. Bumps
    /// [`generation`](Self::generation) only when a value was actually removed.
    pub fn remove<T: Any + 'static>(&mut self) -> Option<T> {
        let entry = self.map.remove(&TypeId::of::<T>())?;
        self.generation += 1;
        entry.value.downcast::<T>().ok().map(|boxed| *boxed)
    }

    /// Return `true` if a value of type `T` is currently stored.
    pub fn contains<T: Any + 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }

    /// Monotonically increasing counter, bumped by [`insert`](Self::insert),
    /// [`get_mut`](Self::get_mut), and a successful [`remove`](Self::remove) (not by [`get`](Self::get)).
    /// Lets consumers (e.g. `build_drawable_snapshot`) detect "nothing changed since I last
    /// looked" without comparing the whole store.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Debug, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[test]
    fn insert_get_round_trips() {
        let mut state = AppState::default();
        state.insert(Point { x: 1, y: 2 });
        assert_eq!(state.get::<Point>(), Some(&Point { x: 1, y: 2 }));
    }

    #[test]
    fn get_mut_round_trips_and_missing_type_returns_none() {
        let mut state = AppState::default();
        state.insert(Point { x: 1, y: 2 });
        if let Some(p) = state.get_mut::<Point>() {
            p.x = 99;
        }
        assert_eq!(state.get::<Point>(), Some(&Point { x: 99, y: 2 }));
        assert!(state.get_mut::<String>().is_none());
    }

    #[test]
    fn remove_takes_value_and_missing_type_returns_none() {
        let mut state = AppState::default();
        state.insert(Point { x: 1, y: 2 });
        assert_eq!(state.remove::<Point>(), Some(Point { x: 1, y: 2 }));
        assert_eq!(state.get::<Point>(), None);
        assert_eq!(state.remove::<Point>(), None);
    }

    #[test]
    fn contains_reflects_presence() {
        let mut state = AppState::default();
        assert!(!state.contains::<Point>());
        state.insert(Point { x: 0, y: 0 });
        assert!(state.contains::<Point>());
    }

    #[test]
    fn clone_of_plain_value_is_independent() {
        let mut state = AppState::default();
        state.insert(Point { x: 1, y: 2 });
        let mut cloned = state.clone();
        if let Some(p) = cloned.get_mut::<Point>() {
            p.x = 999;
        }
        // Mutating the clone must not affect the original — true value-copy semantics.
        assert_eq!(state.get::<Point>(), Some(&Point { x: 1, y: 2 }));
        assert_eq!(cloned.get::<Point>(), Some(&Point { x: 999, y: 2 }));
    }

    #[test]
    fn clone_of_arc_mutex_wrapped_value_aliases_not_copies() {
        // Documents intentional behavior, not a regression: Arc<Mutex<T>> is the recommended
        // wrapper for shared interior-mutable state (e.g. editor caches). Cloning AppState
        // clones the Arc pointer, so mutating through the Mutex is visible on both sides.
        let mut state = AppState::default();
        state.insert(Arc::new(Mutex::new(Point { x: 1, y: 2 })));
        let cloned = state.clone();
        {
            let shared = state.get::<Arc<Mutex<Point>>>().unwrap();
            shared.lock().unwrap().x = 999;
        }
        let cloned_shared = cloned.get::<Arc<Mutex<Point>>>().unwrap();
        assert_eq!(cloned_shared.lock().unwrap().x, 999);
    }

    #[test]
    fn generation_bumps_on_insert_get_mut_and_successful_remove_not_on_get() {
        let mut state = AppState::default();
        assert_eq!(state.generation(), 0);

        state.insert(Point { x: 1, y: 2 });
        assert_eq!(state.generation(), 1);

        let _ = state.get::<Point>();
        assert_eq!(state.generation(), 1, "get() must not bump generation");

        let _ = state.get_mut::<Point>();
        assert_eq!(state.generation(), 2);

        let _ = state.get_mut::<String>();
        assert_eq!(
            state.generation(),
            3,
            "get_mut() bumps even on a missing type"
        );

        assert!(state.remove::<String>().is_none());
        assert_eq!(
            state.generation(),
            3,
            "remove() of a missing type must not bump generation"
        );

        assert!(state.remove::<Point>().is_some());
        assert_eq!(state.generation(), 4);
    }
}
