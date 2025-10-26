//! Simulation time resource.
//!
//! Tracks elapsed time and per-frame delta used by movement, animation, and
//! other time-based systems. `time_scale` can be used for slow-motion effects.

use bevy_ecs::prelude::Resource;

/// World time accumulator and frame delta.
#[derive(Resource, Clone, Copy)]
pub struct WorldTime {
    /// Total elapsed time since start (seconds).
    pub elapsed: f32,
    /// Unscaled delta time for the last frame (seconds).
    pub delta: f32,
    /// Multiplier applied by systems that honor time scaling.
    pub time_scale: f32,
}

impl Default for WorldTime {
    fn default() -> Self {
        WorldTime {
            elapsed: 0.0,
            delta: 0.0,
            time_scale: 1.0,
        }
    }
}

impl WorldTime {
    /* pub fn delta_seconds(&self) -> f32 {
        self.delta * self.time_scale
    } */
}
