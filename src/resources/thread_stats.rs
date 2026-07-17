//! Logic-world per-thread tick-stats resources, surfaced in the F11 debug
//! overlay's perf panel via `DebugSnapshot`.

use bevy_ecs::prelude::Resource;

use crate::protocol::stats::ThreadStats;

/// Sim thread's own tick stats, plus the raw-input backlog gauge -- both
/// collected by the same per-tick `StatsWindow` in `logic_thread_main`,
/// since backlog length has no audio/render analog and doesn't belong in
/// the generic `ThreadStats` shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct SimStats {
    pub thread: ThreadStats,
    /// Average raw-input samples drained per sim tick over the window.
    pub input_backlog_avg: f32,
    /// Largest single-tick raw-input backlog seen in the window.
    pub input_backlog_max: u32,
}

/// Audio thread's tick stats, mirrored logic-side from `AudioMessage::Stats`
/// by `land_audio_stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct AudioStats(pub ThreadStats);
