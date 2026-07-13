//! Fixed-rate loop pacing with a variable-dt fallback (Phase 7b).
//!
//! A [`Pacer`] targets a constant tick period but tolerates OS scheduling
//! jitter and overruns: it returns the *actual* elapsed time each tick so
//! systems can integrate with a real delta rather than assuming a perfect dt.
//! Used by both the logic thread (`[simulation] hz`) and the audio thread
//! (`[audio] hz`) — see `docs/plans/phase7b-pacing-configurable-frequencies.md`.

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, TryRecvError};

/// True once every sender for `rx` has been dropped.
///
/// `Receiver::try_iter()` can't distinguish "nothing queued right now" from
/// "channel disconnected" -- callers drain with `try_iter()` first, then
/// call this once to detect the latter and exit their loop.
pub fn channel_disconnected<T>(rx: &Receiver<T>) -> bool {
    matches!(rx.try_recv(), Err(TryRecvError::Disconnected))
}

/// Paces a loop to a target frequency, returning the real elapsed dt.
pub struct Pacer {
    period: Duration,
    last: Instant,
}

impl Pacer {
    /// Create a pacer targeting `hz` iterations per second.
    pub fn new(hz: f64) -> Self {
        Self {
            period: Duration::from_secs_f64(1.0 / hz),
            last: Instant::now(),
        }
    }

    /// Sleep until the target period has elapsed, then return the real dt
    /// (in seconds) since the previous call.
    pub fn tick(&mut self) -> f32 {
        let elapsed = self.last.elapsed();
        if elapsed < self.period {
            spin_sleep::sleep(self.period - elapsed);
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        dt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_matches_hz() {
        let pacer = Pacer::new(240.0);
        assert_eq!(pacer.period, Duration::from_secs_f64(1.0 / 240.0));
    }

    #[test]
    fn period_matches_low_hz() {
        let pacer = Pacer::new(15.0);
        assert_eq!(pacer.period, Duration::from_secs_f64(1.0 / 15.0));
    }

    #[test]
    fn tick_returns_plausible_positive_dt() {
        // Loose smoke test only (avoid CI flakiness from tight timing
        // assertions): a tick at a modest rate should return a positive dt
        // no larger than a couple of periods, even under scheduler jitter.
        let mut pacer = Pacer::new(1000.0);
        let dt = pacer.tick();
        assert!(dt > 0.0);
        assert!(dt < 0.1, "dt unexpectedly large: {dt}");
    }

    #[test]
    fn tick_dt_reflects_elapsed_time_between_calls() {
        let mut pacer = Pacer::new(1000.0);
        pacer.tick();
        std::thread::sleep(Duration::from_millis(5));
        let dt = pacer.tick();
        // The pacer sleeps to fill the period, then measures real elapsed
        // time from `last` -- an externally-added 5ms sleep before the
        // second tick should be reflected (not truncated away).
        assert!(dt >= 0.004, "dt too small: {dt}");
    }
}
