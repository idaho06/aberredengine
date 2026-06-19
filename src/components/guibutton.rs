//! Clickable themed GUI button.
//!
//! [`GuiButton`] is a dedicated component (size, interaction state, callback
//! chain) — distinct from its caption, which is a separate child
//! [`DynamicText`](super::dynamictext::DynamicText) entity (`ChildOf` +
//! `GuiOffset`), mirroring [`Menu`](super::menu::Menu)'s existing precedent
//! of rendering captions as separate child entities rather than baking text
//! into the widget component. See `docs/gui-system-architecture.md`.

use bevy_ecs::prelude::{Component, Entity};
use raylib::prelude::Vector2;

use crate::systems::GameCtx;

/// Visual/interaction state of a [`GuiButton`], resolved each frame by
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

/// Type alias for a Rust button click callback.
///
/// Mirrors [`MenuRustCallback`](super::menu::MenuRustCallback)'s shape: the
/// click edge itself is the signal, so no raw input access is needed beyond
/// `GameCtx`'s existing commands/queries/resources.
pub type GuiButtonRustCallback = for<'w, 's> fn(Entity, &mut GameCtx<'w, 's>);

/// Clickable themed button, rendered via the global `GuiTheme`'s button skin
/// and resolved each frame by `gui_hit_test_system`/`gui_button_click_observer`.
#[derive(Component, Clone, Debug)]
pub struct GuiButton {
    pub size: Vector2,
    pub state: GuiWidgetState,
    /// Lua callback name, checked first.
    pub on_click_callback: Option<String>,
    /// Rust fn-pointer callback, checked second.
    pub on_rust_callback: Option<GuiButtonRustCallback>,
}

impl GuiButton {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vector2::new(width, height),
            state: GuiWidgetState::Normal,
            on_click_callback: None,
            on_rust_callback: None,
        }
    }

    /// Create a button with a Rust function pointer callback.
    ///
    /// Prefer this over `::new` + `with_on_rust_callback` for Rust callbacks:
    /// the typed parameter forces coercion from the function-item type to
    /// the `fn(...)` pointer type `Query<&GuiButton>` expects. Without the
    /// coercion the query silently matches nothing. Mirrors
    /// `CollisionRule::rust`/`Timer::rust`.
    pub fn rust(width: f32, height: f32, callback: GuiButtonRustCallback) -> Self {
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
    fn test_guibutton_new_defaults() {
        let b = GuiButton::new(80.0, 24.0);
        assert!((b.size.x - 80.0).abs() < f32::EPSILON);
        assert!((b.size.y - 24.0).abs() < f32::EPSILON);
        assert_eq!(b.state, GuiWidgetState::Normal);
        assert!(b.on_click_callback.is_none());
        assert!(b.on_rust_callback.is_none());
    }

    #[test]
    fn test_guibutton_rust_sets_callback() {
        let b = GuiButton::rust(80.0, 24.0, dummy_callback);
        assert!(b.on_rust_callback.is_some());
    }

    #[test]
    fn test_guibutton_with_on_click_callback() {
        let b = GuiButton::new(80.0, 24.0).with_on_click_callback("on_start_clicked");
        assert_eq!(b.on_click_callback.as_deref(), Some("on_start_clicked"));
    }

    #[test]
    fn test_guibutton_with_disabled() {
        let b = GuiButton::new(80.0, 24.0).with_disabled();
        assert_eq!(b.state, GuiWidgetState::Disabled);
    }
}
