//! Shared hit-test/click state for clickable GUI widgets.
//!
//! [`GuiInteractable`] is the shared hit-test/click runtime state for
//! clickable GUI widgets — extracted out of `GuiButton` so a second
//! clickable widget (`GuiImage`) can reuse `gui_hit_test_system`/the
//! click-dispatch observer without duplicating the winner-resolution
//! algorithm. `GuiButton`/`GuiImage` still carry their own full spawn-time
//! data (size, caption/tex_key, callback_name, theme_key); the
//! `gui_button_spawn_system`/`gui_image_spawn_system` reactive spawn systems
//! (`systems/gui_spawn.rs`) react on `Added<GuiButton>`/`Added<GuiImage>` to
//! insert the co-located `GuiInteractable` one frame later. See
//! `docs/gui-system-architecture.md`.

use bevy_ecs::prelude::{Component, Entity};
use raylib::prelude::Vector2;

use crate::systems::GameCtx;

/// Visual/interaction state of a [`GuiInteractable`], resolved each frame by
/// `gui_hit_test_system` from cursor position + the raw left mouse button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GuiWidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    /// Persistent state, never overwritten by hit-test resolution. No
    /// public disable/enable API ships in this slice — set/cleared only by
    /// mutating the component directly (see Open Item #1 in the design doc).
    Disabled,
}

/// Type alias for a Rust click callback. Generalized from the former
/// `GuiButtonRustCallback` — same shape, now widget-agnostic.
///
/// Mirrors [`MenuRustCallback`](super::menu::MenuRustCallback)'s shape: the
/// click edge itself is the signal, so no raw input access is needed beyond
/// `GameCtx`'s existing commands/queries/resources.
pub type GuiRustCallback = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>);

/// Hit-test/click state shared by every clickable GUI widget (`GuiButton`,
/// `GuiImage`, future widgets).
#[derive(Component, Clone, Debug)]
pub struct GuiInteractable {
    pub size: Vector2,
    pub state: GuiWidgetState,
    /// Lua callback name, checked first.
    pub on_click_callback: Option<String>,
    /// Rust fn-pointer callback, checked second.
    pub on_rust_callback: Option<GuiRustCallback>,
}

impl GuiInteractable {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
            state: GuiWidgetState::Normal,
            on_click_callback: None,
            on_rust_callback: None,
        }
    }

    /// Create an interactable with a Rust function pointer callback.
    ///
    /// Prefer this over `::new` + `with_on_rust_callback` for Rust callbacks:
    /// the typed parameter forces coercion from the function-item type to
    /// the `fn(...)` pointer type `Query<&GuiInteractable>` expects. Without
    /// the coercion the query silently matches nothing. Mirrors
    /// `CollisionRule::rust`/`Timer::rust`.
    pub fn rust(width: f32, height: f32, callback: GuiRustCallback) -> Self {
        Self {
            on_rust_callback: Some(callback),
            ..Self::new(width, height)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_callback(_entity: Entity, _ctx: &mut GameCtx) {}

    #[test]
    fn test_guiwidgetstate_default_is_normal() {
        assert_eq!(GuiWidgetState::default(), GuiWidgetState::Normal);
    }

    #[test]
    fn test_guiinteractable_new_defaults() {
        let i = GuiInteractable::new(80.0, 24.0);
        assert!((i.size.x - 80.0).abs() < f32::EPSILON);
        assert!((i.size.y - 24.0).abs() < f32::EPSILON);
        assert_eq!(i.state, GuiWidgetState::Normal);
        assert!(i.on_click_callback.is_none());
        assert!(i.on_rust_callback.is_none());
    }

    #[test]
    fn test_guiinteractable_rust_sets_callback() {
        let i = GuiInteractable::rust(80.0, 24.0, dummy_callback);
        assert!(i.on_rust_callback.is_some());
    }

    #[test]
    fn test_guiinteractable_with_on_click_callback() {
        let i = GuiInteractable::new(80.0, 24.0).with_on_click_callback("on_start_clicked");
        assert_eq!(i.on_click_callback.as_deref(), Some("on_start_clicked"));
    }

    #[test]
    fn test_guiinteractable_with_disabled() {
        let i = GuiInteractable::new(80.0, 24.0).with_disabled();
        assert_eq!(i.state, GuiWidgetState::Disabled);
    }
}
