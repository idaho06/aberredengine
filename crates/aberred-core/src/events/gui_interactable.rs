//! GUI click events.
//!
//! `gui_hit_test_system` triggers a [`GuiClicked`] on a clickable GUI widget
//! (`GuiButton`, `GuiImage`, or any other widget carrying `GuiInteractable`)
//! when it is released while still inside its bounds, having been `Pressed`
//! the preceding frame. Observe it per widget
//! (`commands.spawn(..).observe(handler)`) or globally (`add_observer(handler)`).
//! The GUI counterpart of [`MenuSelected`](super::menu::MenuSelected).

use bevy_ecs::prelude::*;

/// Triggered on a `GuiInteractable` widget when it is clicked
/// (press, then release inside).
#[derive(EntityEvent, Clone, Copy, Debug)]
pub struct GuiClicked {
    /// The clicked widget (a `GuiButton`, `GuiImage`, or any other
    /// `GuiInteractable`-carrying entity).
    #[event_target]
    pub entity: Entity,
}
