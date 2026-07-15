//! Simulation time resource.
//!
//! Tracks elapsed time and per-frame delta used by movement, animation, and
//! other time-based systems. `time_scale` can be used for slow-motion effects.

use bevy_ecs::prelude::Resource;

/// World time accumulator and frame delta.
#[derive(Resource, Clone, Copy, Debug)]
pub struct WorldTime {
    /// Total elapsed time since start (seconds).
    pub elapsed: f32,
    /// Scaled delta time for the last update (seconds): the real elapsed
    /// time since the previous sim tick (as measured by the logic thread's
    /// `Pacer`, targeting `[simulation] hz` in `config.ini`), clamped
    /// (`DT_CLAMP_SECONDS`) and multiplied by `time_scale`. One tick, one
    /// delta — there is no separate "fixed substep" vs. "variable frame"
    /// delta.
    pub delta: f32,
    /// Multiplier applied by systems that honor time scaling.
    pub time_scale: f32,
    /// Total number of frames since start.
    pub frame_count: u64,
}

impl Default for WorldTime {
    fn default() -> Self {
        WorldTime {
            elapsed: 0.0,
            delta: 0.0,
            time_scale: 1.0,
            frame_count: 0,
        }
    }
}

impl WorldTime {
    pub fn with_time_scale(mut self, scale: f32) -> Self {
        self.time_scale = scale;
        self
    }
}
