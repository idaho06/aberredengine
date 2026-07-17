//! Per-thread tick/frame timing stats, rolled up over a ~1s window by
//! [`crate::pacing::StatsWindow`] and surfaced in the F11 debug overlay's
//! perf panel.

/// One thread's loop health over the last sample window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThreadStats {
    /// The thread's configured rate (`sim_hz`/`audio_hz`/`target_fps`).
    pub configured_hz: f32,
    /// Ticks actually completed in the window, divided by the window's real
    /// elapsed time.
    pub achieved_hz: f32,
    /// Average time spent in the timed work per tick (not the pacer sleep).
    pub tick_avg_ms: f32,
    /// Slowest tick's work time in the window.
    pub tick_max_ms: f32,
    /// Ticks in the window whose work time exceeded the configured period
    /// (the pacer couldn't sleep at all that tick).
    pub overruns: u32,
    /// Ticks completed in the window (the same count `achieved_hz` divides
    /// from). Exposed so callers tracking a per-tick gauge alongside the
    /// timed work (e.g. the sim thread's input backlog) can average it over
    /// the same window without keeping their own shadow tick counter in
    /// lockstep with `StatsWindow`'s internal one.
    pub ticks: u32,
}
