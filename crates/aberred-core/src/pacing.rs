//! Fixed-rate loop pacing with a variable-dt fallback.
//!
//! A [`Pacer`] targets a constant tick period but tolerates OS scheduling
//! jitter and overruns: it returns the *actual* elapsed time each tick so
//! systems can integrate with a real delta rather than assuming a perfect dt.
//! Used by both the logic thread (`[simulation] hz`) and the audio thread
//! (`[audio] hz`).

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError};

use crate::protocol::stats::ThreadStats;

/// True once every sender for `rx` has been dropped.
///
/// `Receiver::try_iter()` can't distinguish "nothing queued right now" from
/// "channel disconnected" -- callers drain with `try_iter()` first, then
/// call this once to detect the latter and exit their loop.
pub fn channel_disconnected<T>(rx: &Receiver<T>) -> bool {
    matches!(rx.try_recv(), Err(TryRecvError::Disconnected))
}

/// Sender-side counterpart to [`channel_disconnected`]: true when a
/// `try_send` result means the receiver is gone, as opposed to the channel
/// merely being momentarily full (expected, non-fatal backpressure on a
/// bounded channel -- see `LogicBridge::tx_input`).
pub fn send_channel_disconnected<T>(result: &Result<(), TrySendError<T>>) -> bool {
    matches!(result, Err(TrySendError::Disconnected(_)))
}

/// Sends `value` on `tx`; if the bounded channel is momentarily full, pops
/// exactly one stale entry off `rx` (the oldest still queued) and retries
/// the send once, so the freshest value wins instead of being silently
/// discarded. Correct only with a single producer on `tx` -- see
/// `LogicBridge::rx_input`'s doc comment for the invariant this relies on;
/// with multiple producers another sender could refill the freed slot
/// between the pop and the retry, race-losing the retry back to `Full`.
pub fn send_or_drop_oldest<T>(
    tx: &Sender<T>,
    rx: &Receiver<T>,
    value: T,
) -> Result<(), TrySendError<T>> {
    match tx.try_send(value) {
        Err(TrySendError::Full(value)) => {
            let _ = rx.try_recv();
            tx.try_send(value)
        }
        result => result,
    }
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

    /// The configured tick period, in seconds. Callers integrating a
    /// constant fixed `dt` (see [`tick_fixed`](Self::tick_fixed)) should
    /// derive it from here rather than re-deriving `1.0 / hz` independently,
    /// so there is exactly one source of truth for the period.
    pub fn period_secs_f32(&self) -> f32 {
        self.period.as_secs_f32()
    }

    /// Sleep until the target period has elapsed since `self.last`. Shared
    /// by [`tick`](Self::tick) and [`tick_fixed`](Self::tick_fixed), which
    /// diverge only in how they advance `self.last` afterward.
    fn sleep_to_deadline(&self) {
        let elapsed = self.last.elapsed();
        if elapsed < self.period {
            spin_sleep::sleep(self.period - elapsed);
        }
    }

    /// Sleep until the target period has elapsed, then return the real dt
    /// (in seconds) since the previous call.
    pub fn tick(&mut self) -> f32 {
        self.sleep_to_deadline();
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        dt
    }

    /// Sleep until the target period has elapsed (same discipline as
    /// [`tick`](Self::tick)), but advance the deadline by exactly one
    /// `period` (carrying any overshoot forward) instead of resetting it to
    /// `Instant::now()` -- a reset would systematically undershoot the
    /// configured rate, since the overshoot past each period would be
    /// discarded every call. If still behind by a full period after
    /// advancing (e.g. after a long stall or debugger pause), the deadline
    /// snaps to now instead of the caller getting a burst of unpaced calls
    /// to catch up.
    ///
    /// Returns nothing: callers in fixed-timestep mode must integrate a
    /// constant, config-derived period (`1.0 / hz`), never a measured value.
    /// This is the sleeping
    /// counterpart to the pre-`f02581c` `Pacer::due()` (deleted when PRESENT
    /// decimation moved to the wall-clock-agnostic
    /// [`TickCountdown`](TickCountdown)); `due()`'s deadline-carry + snap
    /// arithmetic is reused here, but `due()` itself never slept (it was a
    /// non-blocking decimator check inside an already-paced loop) --
    /// `tick_fixed` paces the loop itself, so it must sleep too.
    pub fn tick_fixed(&mut self) {
        self.sleep_to_deadline();
        self.last += self.period;
        if self.last.elapsed() >= self.period {
            self.last = Instant::now();
        }
    }

    /// Advance the deadline to now without sleeping. Used by replay
    /// fast-forward: the sim still integrates the same fixed `dt` every tick
    /// (`period_secs_f32`), just without waiting for wall-clock time to pass
    /// between ticks. Never called from the live/non-replay path.
    pub fn skip_to_now(&mut self) {
        self.last = Instant::now();
    }
}

/// Non-blocking, tick-count-based decimator: fires once every `skip + 1`
/// calls to [`due`](Self::due). Unlike [`Pacer`], never sleeps and isn't
/// wall-clock paced -- for decimating a less-frequent task (e.g. publishing
/// a snapshot) against the tick count of a loop already paced by a `Pacer`,
/// rather than against real elapsed time.
pub struct TickCountdown {
    skip: u32,
    remaining: u32,
}

impl TickCountdown {
    /// Create a countdown that fires immediately on the first `due()` call,
    /// then every `skip + 1` calls thereafter.
    pub fn new(skip: u32) -> Self {
        Self { skip, remaining: 0 }
    }

    /// Non-blocking check: is this call due to fire? Decrements the
    /// countdown otherwise.
    pub fn due(&mut self) -> bool {
        if self.remaining == 0 {
            self.remaining = self.skip;
            true
        } else {
            self.remaining -= 1;
            false
        }
    }
}

/// Rolls up per-tick work-time measurements into a [`ThreadStats`] snapshot
/// once per ~1s window. Measures only the timed work handed to `record`
/// (e.g. a schedule run) -- never the pacer's own sleep.
pub struct StatsWindow {
    configured_hz: f32,
    period: Duration,
    window: Duration,
    window_start: Instant,
    ticks: u32,
    total_work: Duration,
    max_work: Duration,
    overruns: u32,
}

impl StatsWindow {
    /// Create a window targeting `hz` and rolling over every ~1s.
    pub fn new(hz: f64) -> Self {
        Self::with_window(hz, Duration::from_secs(1))
    }

    /// Create a window targeting `hz` with an explicit rollover period.
    /// Production code should use [`Self::new`]; a shorter `window` is only
    /// useful in tests, so real rollovers don't require a real 1s sleep.
    pub fn with_window(hz: f64, window: Duration) -> Self {
        Self {
            configured_hz: hz as f32,
            period: Duration::from_secs_f64(1.0 / hz),
            window,
            window_start: Instant::now(),
            ticks: 0,
            total_work: Duration::ZERO,
            max_work: Duration::ZERO,
            overruns: 0,
        }
    }

    /// Record one tick's work time (excluding any pacer sleep). Returns
    /// `Some(ThreadStats)` once the window has rolled over, resetting the
    /// accumulators for the next window; otherwise `None`.
    pub fn record(&mut self, work_time: Duration) -> Option<ThreadStats> {
        self.ticks += 1;
        self.total_work += work_time;
        self.max_work = self.max_work.max(work_time);
        if work_time > self.period {
            self.overruns += 1;
        }

        let elapsed = self.window_start.elapsed();
        if elapsed < self.window {
            return None;
        }

        let stats = ThreadStats {
            configured_hz: self.configured_hz,
            achieved_hz: if elapsed.as_secs_f32() > 0.0 {
                self.ticks as f32 / elapsed.as_secs_f32()
            } else {
                0.0
            },
            tick_avg_ms: if self.ticks > 0 {
                self.total_work.as_secs_f32() * 1000.0 / self.ticks as f32
            } else {
                0.0
            },
            tick_max_ms: self.max_work.as_secs_f32() * 1000.0,
            overruns: self.overruns,
            ticks: self.ticks,
        };

        self.window_start = Instant::now();
        self.ticks = 0;
        self.total_work = Duration::ZERO;
        self.max_work = Duration::ZERO;
        self.overruns = 0;

        Some(stats)
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

    #[test]
    fn tick_fixed_carries_overshoot_forward() {
        // At a fast rate, the actual sleep+measure loop will overshoot the
        // period somewhat every call; the deadline should still advance by
        // exactly one period per call rather than resetting to `now()`,
        // which would systematically undershoot the configured rate.
        let mut pacer = Pacer::new(1000.0);
        let first_deadline = pacer.last + pacer.period;
        pacer.tick_fixed();
        assert_eq!(
            pacer.last, first_deadline,
            "deadline should advance by exactly one period, not reset to now"
        );
    }

    #[test]
    fn tick_fixed_stall_snaps_once() {
        let mut pacer = Pacer::new(1000.0);
        // Simulate a long stall by backdating `last` well behind now.
        pacer.last -= Duration::from_millis(50);
        let before = Instant::now();
        pacer.tick_fixed();
        assert!(
            pacer.last >= before,
            "stall guard should snap the deadline close to now, not leave it periods behind"
        );
        // Immediately re-checking after the stall snap must not still be
        // behind by a full period (i.e. must not fire a catch-up burst).
        assert!(pacer.last.elapsed() < pacer.period);
    }

    #[test]
    fn send_or_drop_oldest_sends_directly_when_not_full() {
        let (tx, rx) = crossbeam_channel::bounded::<i32>(2);
        assert!(send_or_drop_oldest(&tx, &rx, 1).is_ok());
        assert_eq!(rx.try_recv(), Ok(1));
    }

    #[test]
    fn send_or_drop_oldest_drops_oldest_when_full() {
        let (tx, rx) = crossbeam_channel::bounded::<i32>(2);
        tx.try_send(1).unwrap();
        tx.try_send(2).unwrap();
        assert!(send_or_drop_oldest(&tx, &rx, 3).is_ok());
        // Oldest (1) was dropped to make room; 2 and 3 remain, in order.
        assert_eq!(rx.try_recv(), Ok(2));
        assert_eq!(rx.try_recv(), Ok(3));
    }

    #[test]
    fn send_or_drop_oldest_reports_disconnected() {
        let (tx, rx) = crossbeam_channel::bounded::<i32>(1);
        drop(rx);
        let rx2 = crossbeam_channel::bounded::<i32>(1).1; // unrelated, unused receiver
        let result = send_or_drop_oldest(&tx, &rx2, 1);
        assert!(send_channel_disconnected(&result));
    }

    #[test]
    fn stats_window_does_not_roll_over_before_window_elapses() {
        let mut window = StatsWindow::with_window(1000.0, Duration::from_secs(10));
        assert!(window.record(Duration::from_micros(100)).is_none());
    }

    #[test]
    fn stats_window_rolls_over_after_window_elapses() {
        let mut window = StatsWindow::with_window(1000.0, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(6));
        let stats = window
            .record(Duration::from_micros(100))
            .expect("window should have rolled over");
        assert_eq!(stats.configured_hz, 1000.0);
        assert!(stats.achieved_hz > 0.0);
        assert!(stats.tick_avg_ms > 0.0);
        assert!(stats.tick_max_ms > 0.0);
    }

    #[test]
    fn stats_window_counts_overruns() {
        // period is 100ms at 10hz; a 200ms "tick" overruns it.
        let mut window = StatsWindow::with_window(10.0, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(6));
        let stats = window
            .record(Duration::from_millis(200))
            .expect("window should have rolled over");
        assert_eq!(stats.overruns, 1);
    }

    #[test]
    fn stats_window_no_overrun_when_work_within_period() {
        let mut window = StatsWindow::with_window(10.0, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(6));
        let stats = window
            .record(Duration::from_millis(1))
            .expect("window should have rolled over");
        assert_eq!(stats.overruns, 0);
    }

    #[test]
    fn stats_window_resets_accumulators_after_rollover() {
        let mut window = StatsWindow::with_window(10.0, Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(6));
        let first = window
            .record(Duration::from_millis(200))
            .expect("first window should roll over");
        assert_eq!(first.overruns, 1);

        // Immediately after a rollover the new window shouldn't fire again
        // until its own period elapses, and shouldn't carry over the prior
        // window's overrun/max-work accumulation.
        assert!(window.record(Duration::from_micros(1)).is_none());
        std::thread::sleep(Duration::from_millis(6));
        let second = window
            .record(Duration::from_micros(1))
            .expect("second window should roll over");
        assert_eq!(
            second.overruns, 0,
            "overrun count must not carry over from the previous window"
        );
        assert!(
            second.tick_max_ms < first.tick_max_ms,
            "max work must reset, not carry the previous window's 200ms spike"
        );
    }

    #[test]
    fn stats_window_tick_avg_reflects_multiple_ticks() {
        let mut window = StatsWindow::with_window(1000.0, Duration::from_millis(5));
        window.record(Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(6));
        let stats = window
            .record(Duration::from_millis(3))
            .expect("window should have rolled over");
        // avg of two ticks (1ms, 3ms) is 2ms
        assert!(
            (stats.tick_avg_ms - 2.0).abs() < 0.5,
            "avg unexpectedly far from 2ms: {}",
            stats.tick_avg_ms
        );
        assert!(
            (stats.tick_max_ms - 3.0).abs() < 0.5,
            "max unexpectedly far from 3ms: {}",
            stats.tick_max_ms
        );
        assert_eq!(stats.ticks, 2);
    }
}
