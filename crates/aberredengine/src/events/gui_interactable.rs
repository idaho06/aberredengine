//! GUI interactable click events.
//!
//! [`GuiInteractableClickEvent`] is triggered when any clickable GUI widget
//! (`GuiButton`, `GuiImage`, or any other widget carrying `GuiInteractable`) is
//! released while still inside its bounds, having been `Pressed` the
//! preceding frame. Mirrors [`MenuSelectionEvent`](super::menu::MenuSelectionEvent).
//! `gui_interactable_click_observer` dispatches the matching Lua/Rust
//! callback chain for the clicked entity.

use bevy_ecs::prelude::*;

/// Event triggered when a `GuiInteractable`'s press-then-release-inside
/// transition is detected by `gui_hit_test_system`.
///
/// Observed by `gui_interactable_click_observer`, which dispatches the
/// Lua/Rust callback chain.
#[derive(Event, Debug, Clone)]
pub struct GuiInteractableClickEvent {
    /// The entity that was clicked (a `GuiButton`, `GuiImage`, or any other
    /// `GuiInteractable`-carrying widget).
    pub entity: Entity,
}
