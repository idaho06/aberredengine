//! GUI button click events.
//!
//! This module provides the [`GuiButtonClickEvent`] which is triggered when
//! a `GuiButton` is released while still inside its bounds, having been
//! `Pressed` the preceding frame. Mirrors [`MenuSelectionEvent`](super::menu::MenuSelectionEvent).

use bevy_ecs::prelude::*;

/// Event triggered when a `GuiButton`'s press-then-release-inside transition
/// is detected by `gui_hit_test_system`.
///
/// Observed by `gui_button_click_observer`, which dispatches the Lua/Rust
/// callback chain.
#[derive(Event, Debug, Clone)]
pub struct GuiButtonClickEvent {
    /// The button entity that was clicked.
    pub button: Entity,
}
