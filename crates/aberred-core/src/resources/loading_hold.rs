//! [`LoadingHold`]: sim ticks `Setup` must last so the engine loading screen
//! stays on screen long enough to read.

use bevy_ecs::prelude::Resource;

/// Sim ticks left before `Setup` may end, counted down while the engine
/// loading screen is shown. `finish_setup` waits for it like it waits for
/// [`PendingAssets`](crate::resources::pending_assets::PendingAssets).
///
/// Counted in sim ticks, not wall time: `WorldTime` is frozen in `Setup`, and
/// a tick count stays deterministic.
#[derive(Resource, Debug, PartialEq, Eq)]
pub struct LoadingHold {
    remaining: u32,
}

impl LoadingHold {
    /// A hold of `seconds` at `sim_hz` ticks per second, rounded up.
    pub fn for_seconds(seconds: f32, sim_hz: f64) -> Self {
        Self {
            remaining: (f64::from(seconds) * sim_hz).ceil().max(0.0) as u32,
        }
    }

    /// Counts one sim tick down; stays at zero once elapsed.
    pub fn tick(&mut self) {
        self.remaining = self.remaining.saturating_sub(1);
    }

    /// Whether the hold has elapsed.
    pub fn is_done(&self) -> bool {
        self.remaining == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_lasts_seconds_times_hz_ticks_rounded_up() {
        let mut hold = LoadingHold::for_seconds(0.5, 15.0);
        for _ in 0..7 {
            hold.tick();
            assert!(!hold.is_done());
        }
        hold.tick();
        assert!(hold.is_done());
    }

    #[test]
    fn a_zero_hold_is_done_at_once_and_ticking_stays_done() {
        let mut hold = LoadingHold::for_seconds(0.0, 240.0);
        assert!(hold.is_done());
        hold.tick();
        assert!(hold.is_done());
    }
}
