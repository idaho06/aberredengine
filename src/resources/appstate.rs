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

/// Generic typed state store for ECS-to-GUI communication.
///
/// See the [module documentation](self) for usage guidelines.
#[derive(Resource, Default)]
pub struct AppState {
    map: FxHashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl AppState {
    /// Store a value. Replaces any previous value of type `T`.
    pub fn insert<T: Any + Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Return a shared reference to the stored value of type `T`, or `None`.
    pub fn get<T: Any + 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())?.downcast_ref::<T>()
    }

    /// Return an exclusive reference to the stored value of type `T`, or `None`.
    pub fn get_mut<T: Any + 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())?.downcast_mut::<T>()
    }

    /// Remove and return the stored value of type `T`, or `None`.
    pub fn remove<T: Any + 'static>(&mut self) -> Option<T> {
        self.map
            .remove(&TypeId::of::<T>())
            .and_then(|v| v.downcast::<T>().ok())
            .map(|v| *v)
    }

    /// Return `true` if a value of type `T` is currently stored.
    pub fn contains<T: Any + 'static>(&self) -> bool {
        self.map.contains_key(&TypeId::of::<T>())
    }
}
