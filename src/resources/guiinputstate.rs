//! Per-frame scratch state for GUI input resolution.
//!
//! [`GuiInputState`] is reset at the start of `gui_hit_test_system` (first in
//! the per-frame GUI ordering chain) and read by gameplay systems that want
//! to avoid double-handling a click GUI already consumed this frame. See
//! `docs/gui-system-architecture.md`'s "Click Consumption" section.

use bevy_ecs::prelude::Resource;

/// Tracks whether a click has already been consumed by GUI this frame.
#[derive(Resource, Default, Debug)]
pub struct GuiInputState {
    pub click_consumed_this_frame: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guiinputstate_default_not_consumed() {
        let state = GuiInputState::default();
        assert!(!state.click_consumed_this_frame);
    }
}
