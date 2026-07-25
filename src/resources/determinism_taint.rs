//! Deterministic-session integrity flag.
//!
//! [`DeterminismTaint`] is set when the logic thread detects a sim-visible
//! event in deterministic mode that its recorded `TickInput` envelope
//! cannot account for -- today, only the async-asset preload guard (v1 of
//! `docs/plans/determinism-04-tick-input.md` §4: a `TextureDimsStore` key
//! arriving for the first time while `GameState::Playing`, a proxy for
//! "asset metadata loaded outside the preload window"). A tainted session's
//! replay/lockstep recording (later determinism-roadmap phases) should not
//! be trusted as bit-exact reproducible.

use bevy_ecs::prelude::Resource;

/// Whether this logic-thread session has hit a known determinism hazard.
/// Inserted unconditionally in `setup_logic_world` (always `false` in
/// non-deterministic mode, since nothing ever taints it there) so no system
/// needs to special-case its absence.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DeterminismTaint {
    tainted: bool,
}

impl DeterminismTaint {
    /// Mark the session tainted. Idempotent -- once tainted, stays tainted.
    pub fn taint(&mut self) {
        self.tainted = true;
    }

    /// Whether the session has been tainted.
    pub fn is_tainted(&self) -> bool {
        self.tainted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_not_tainted() {
        assert!(!DeterminismTaint::default().is_tainted());
    }

    #[test]
    fn taint_sets_flag_and_is_idempotent() {
        let mut taint = DeterminismTaint::default();
        taint.taint();
        assert!(taint.is_tainted());
        taint.taint();
        assert!(taint.is_tainted());
    }
}
