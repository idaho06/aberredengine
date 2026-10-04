//! Shared hit-test/click state for clickable GUI widgets.
//!
//! [`GuiInteractable`] is the shared hit-test/click runtime state for
//! clickable GUI widgets, letting `gui_hit_test_system` serve any clickable
//! widget (`GuiButton`, `GuiImage`) without duplicating the
//! winner-resolution algorithm. `GuiButton`/`GuiImage` still carry their own
//! full spawn-time data (size, caption/tex_key, callback_name, theme_key); the
//! `gui_button_spawn_system`/`gui_image_spawn_system` reactive spawn systems
//! (`systems/gui_spawn.rs`) react on `Added<GuiButton>`/`Added<GuiImage>` to
//! insert the co-located `GuiInteractable` one frame later.

use crate::math::Vec2;
use bevy_ecs::prelude::Component;

/// Visual/interaction state of a [`GuiInteractable`], resolved each frame by
/// `gui_hit_test_system` from cursor position + the raw left mouse button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiWidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    /// Persistent state, never overwritten by hit-test resolution. Set or
    /// clear it by mutating the component directly.
    Disabled,
}

/// Hit-test/click state shared by every clickable GUI widget (`GuiButton`,
/// `GuiImage`, and any other widget that carries `GuiInteractable`).
///
/// A click (press then release inside the widget) triggers the
/// [`GuiClicked`](crate::events::gui_interactable::GuiClicked) event on this
/// entity; Rust code observes it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct GuiInteractable {
    pub size: Vec2,
    pub state: GuiWidgetState,
    /// Lua callback name, called on click when the game runs a Lua script.
    pub on_click_callback: Option<String>,
}

impl GuiInteractable {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
            state: GuiWidgetState::Normal,
            on_click_callback: None,
        }
    }

    pub fn with_on_click_callback(mut self, callback: impl Into<String>) -> Self {
        self.on_click_callback = Some(callback.into());
        self
    }

    pub fn with_disabled(mut self) -> Self {
        self.state = GuiWidgetState::Disabled;
        self
    }
}
