//! Deterministic-session integrity flag.
//!
//! [`DeterminismTaint`] is set when the logic thread detects a sim-visible
//! event in deterministic mode that its recorded `TickInput` envelope
//! cannot account for. Today the guard tracks a `TextureDimsStore` key that
//! arrives for the first time while `GameState::Playing`, which marks asset
//! metadata loading outside the preload window. A tainted session's replay
//! recording is not bit-exact reproducible; the flag rides out in
//! `ReplayEntry::End`'s `tainted` field so a divergence report can
//! distinguish a tainted recording from a code regression.

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
