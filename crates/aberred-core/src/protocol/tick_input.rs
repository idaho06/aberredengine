//! Canonical per-tick sim input record.
//!
//! [`TickInput`] is the recorded fact of what a single sim tick consumed —
//! it IS the replay wire format.
//! This module does not change *what* the sim reads today; it makes the
//! assignment of {raw input samples, signal intents, screen-size changes} to
//! a tick an explicit, recorded value instead of a thread-scheduling
//! accident.

use crate::protocol::raw_input::{ImguiCaptureState, RawDeviceSnapshot};
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

    /// Folds the latched facts of `older`, an earlier tick whose input was
    /// held back, into this one: its `capture`/`screen_size` where this tick
    /// has none, and its `intents` ahead of this tick's. Its `samples` are
    /// dropped: a key held during the earlier tick shows up as pressed on
    /// the next sample this tick carries.
    pub fn absorb_latched(&mut self, older: TickInput) {
        self.capture = self.capture.or(older.capture);
        self.screen_size = self.screen_size.or(older.screen_size);
        self.intents.splice(0..0, older.intents);
    }

    /// True when nothing sim-visible landed this tick.
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
    fn absorbing_an_earlier_tick_keeps_its_latched_facts_but_not_its_samples() {
        let older = TickInput {
            tick: 0,
            samples: vec![RawDeviceSnapshot::default()],
            capture: Some(ImguiCaptureState {
                mouse: true,
                keyboard: false,
            }),
            intents: vec![SignalIntent::SetFlag("first".into())],
            screen_size: Some((640, 480)),
        };
        let mut newer = TickInput {
            tick: 3,
            samples: Vec::new(),
            capture: None,
            intents: vec![SignalIntent::SetFlag("second".into())],
            screen_size: Some((800, 600)),
        };

        newer.absorb_latched(older.clone());

        assert_eq!(newer.tick, 3);
        assert!(
            newer.samples.is_empty(),
            "an earlier tick's samples are dropped"
        );
        assert_eq!(newer.capture, older.capture, "kept when this tick has none");
        assert_eq!(
            newer.screen_size,
            Some((800, 600)),
            "this tick's own value wins"
        );
        assert_eq!(
            newer.intents,
            [
                SignalIntent::SetFlag("first".into()),
                SignalIntent::SetFlag("second".into())
            ],
            "earlier intents apply first"
        );
    }

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
