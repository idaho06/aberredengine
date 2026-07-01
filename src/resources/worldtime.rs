//! Simulation time resource.
//!
//! Tracks elapsed time and per-frame delta used by movement, animation, and
//! other time-based systems. `time_scale` can be used for slow-motion effects.

use bevy_ecs::prelude::Resource;

/// Fixed simulation tick duration in seconds (240 Hz). Used by the fixed-step
/// accumulator loop in `EngineBuilder::main_loop` to advance core simulation
/// systems (movement, collision, phases, animation, ...) deterministically,
/// independent of the render frame rate.
pub const FIXED_DT: f32 = 1.0 / 240.0;

/// World time accumulator and frame delta.
#[derive(Resource, Clone, Copy)]
pub struct WorldTime {
    /// Total elapsed time since start (seconds).
    pub elapsed: f32,
    /// Scaled delta time for the last update (seconds). During a fixed-schedule
    /// substep this is `FIXED_DT * time_scale`; while the variable schedule
    /// runs it is the real render-frame delta `* time_scale`. Systems don't
    /// need to know which — they just read whichever value is current for
    /// the schedule that's running them.
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
