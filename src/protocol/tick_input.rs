//! Canonical per-tick sim input record.
//!
//! [`TickInput`] is the recorded fact of what a single sim tick consumed —
//! it IS the replay wire format and the lockstep wire format (both are later
//! phases of the determinism roadmap; see `docs/plans/determinism-04-tick-input.md`).
//! This module does not change *what* the sim reads today; it makes the
//! assignment of {raw input samples, signal intents, screen-size changes} to
//! a tick an explicit, recorded value instead of a thread-scheduling
//! accident.

use crate::protocol::raw_input::RawDeviceSnapshot;
use crate::resources::render::imgui_bridge::ImguiCaptureState;
use crate::resources::signal_intents::SignalIntent;

/// Everything the sim consumes for exactly one tick, in canonical form.
///
/// `capture`/`screen_size` are `Option` rather than bare values: both mirror
/// "did a change land this tick" — a tick with no new capture/size sample
/// must leave the corresponding world resource untouched (see
/// `apply_tick_input`'s doc comment), not overwrite it with a default.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TickInput {
    /// `WorldTime.frame_count` this applies to, read before that tick's
    /// `update_world_time` call increments it — i.e. 0-indexed, "the tick
    /// about to run."
    pub tick: u64,
    /// Ordered raw samples resolved this tick (often 0 or 1; a backlog when
    /// `sim_hz` trails the render rate or a sim stall occurred).
    pub samples: Vec<RawDeviceSnapshot>,
    /// Newest imgui capture state to land this tick, if any.
    pub capture: Option<ImguiCaptureState>,
    /// `SignalIntent`s queued (render-side `GuiCallback`) that apply this
    /// tick.
    pub intents: Vec<SignalIntent>,
    /// `ScreenSize` change landing this tick, if any.
    pub screen_size: Option<(i32, i32)>,
}

impl TickInput {
    /// Reset in place for `tick`, retaining `samples`/`intents` allocations
    /// across ticks — collected at up to `sim_hz` (default 240/s), same
    /// allocation-avoidance rationale as the old per-tick `input_backlog`
    /// local it replaces.
    pub fn reset(&mut self, tick: u64) {
        self.tick = tick;
        self.samples.clear();
        self.capture = None;
        self.intents.clear();
        self.screen_size = None;
    }

    /// True when nothing sim-visible landed this tick — the case a future
    /// replay format's delta encoding compresses to ~0 bytes.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
            && self.capture.is_none()
            && self.intents.is_empty()
            && self.screen_size.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        assert!(TickInput::default().is_empty());
    }

    #[test]
    fn reset_clears_fields_and_sets_tick() {
        let mut ti = TickInput {
            tick: 5,
            samples: vec![RawDeviceSnapshot::default()],
            capture: Some(ImguiCaptureState::default()),
            intents: vec![SignalIntent::SetFlag("x".into())],
            screen_size: Some((640, 480)),
        };
        ti.reset(9);
        assert_eq!(ti.tick, 9);
        assert!(ti.is_empty());
    }

    #[test]
    fn non_empty_when_any_field_set() {
        let mut ti = TickInput::default();
        ti.samples.push(RawDeviceSnapshot::default());
        assert!(!ti.is_empty());

        let ti = TickInput {
            capture: Some(ImguiCaptureState::default()),
            ..Default::default()
        };
        assert!(!ti.is_empty());

        let mut ti = TickInput::default();
        ti.intents.push(SignalIntent::SetFlag("x".into()));
        assert!(!ti.is_empty());

        let ti = TickInput {
            screen_size: Some((1, 1)),
            ..Default::default()
        };
        assert!(!ti.is_empty());
    }
}
