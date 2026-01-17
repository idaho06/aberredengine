//! Time-to-live component for automatic entity despawning.
//!
//! The [`Ttl`] component counts down time each frame. When the remaining time
//! reaches zero, the entity is automatically despawned. Unlike [`LuaTimer`],
//! there is no callback - it's a "fire and forget" mechanism.
//!
//! # How It Works
//!
//! 1. Entity is spawned with a `Ttl` component containing remaining time
//! 2. The `ttl_system` runs each frame:
//!    - Decrements remaining time by `delta * time_scale`
//!    - When `remaining <= 0`, despawns the entity
//!
//! # Usage from Lua
//!
//! ```lua
//! -- Add TTL during spawn
//! engine.spawn()
//!     :with_position(100, 100)
//!     :with_sprite("bullet", 8, 8, 4, 4)
//!     :with_ttl(5.0)  -- despawns after 5 seconds
//!     :build()
//!
//! -- Add TTL to existing entity
//! engine.entity_insert_ttl(entity_id, 3.0)
//! ```
//!
//! # Related
//!
//! - [`crate::systems::ttl::ttl_system`] – system that updates and despawns entities
//! - [`crate::components::luatimer::LuaTimer`] – for delayed callbacks instead of despawn

use bevy_ecs::prelude::Component;

/// Time-to-live component that automatically despawns entities after a duration.
///
/// The countdown respects [`WorldTime::time_scale`](crate::resources::worldtime::WorldTime)
/// and continues regardless of entity frozen state.
#[derive(Component)]
pub struct Ttl {
    /// Remaining time in seconds before despawn.
    pub remaining: f32,
}

impl Ttl {
    /// Create a new Ttl with the given duration in seconds.
    ///
    /// # Arguments
    ///
    /// * `seconds` - Time in seconds before the entity despawns
    pub fn new(seconds: f32) -> Self {
        Ttl { remaining: seconds }
    }
}
