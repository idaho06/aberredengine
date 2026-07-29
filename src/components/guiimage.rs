//! Clickable image widget for inventory/item-slot UIs.
//!
//! `GuiImage` carries the spawn-time data for the widget: size, texture key,
//! and click callback name. `gui_image_spawn_system`
//! (`systems/gui_spawn.rs`), reacting on `Added<GuiImage>`, inserts a
//! co-located `GuiInteractable` (hit-test/click) and `Sprite` (visual) using
//! that data — rendering reads the `Sprite` (free-rides the engine's
//! existing `screen_sprites` collection in `render/mod.rs`, no new render
//! code), and hit-testing reads the co-located `GuiInteractable.size`.
//!
//! Unlike `GuiButton`/`GuiLabel`, `GuiImage` has NO caption child — the
//! `Sprite` lives on the *same* entity as `GuiImage`/`GuiInteractable`, not
//! a `ChildOf` child.
//!
//! The widget handles click interaction only. Gameplay code applies any
//! hover/press/disabled visual feedback itself (for example by setting
//! `Tint` from a callback), and drag behavior lives outside this component.

use std::sync::Arc;

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Clickable image slot. `gui_image_spawn_system` reacts on
/// `Added<GuiImage>` to insert the co-located `GuiInteractable` + `Sprite`.
#[derive(Component, Clone, Debug)]
pub struct GuiImage {
    pub size: Vector2,
    pub tex_key: String,
    /// Pixel position of the atlas sub-rect within `tex_key` (mirrors
    /// `Sprite.offset`) — `size` doubles as both the source-rect size and
    /// the render size, same convention `Sprite` already uses. This is the
    /// "normal" state's offset; `offset_hover`/`offset_pressed`/
    /// `offset_disabled` fall back to this value when unset.
    pub offset: Vector2,
    /// Atlas offset to use while `GuiInteractable.state == Hovered`. `None`
    /// falls back to `offset` — same "only normal required" convention as
    /// `GuiButtonSkin.hover`. Synced to `Sprite.offset` every frame by
    /// `gui_image_state_sync_system`.
    pub offset_hover: Option<Vector2>,
    /// Atlas offset to use while `GuiInteractable.state == Pressed`. `None`
    /// falls back to `offset`.
    pub offset_pressed: Option<Vector2>,
    /// Atlas offset to use while `GuiInteractable.state == Disabled`. `None`
    /// falls back to `offset`.
    pub offset_disabled: Option<Vector2>,
    /// Lua callback name, checked first by the click dispatch chain. Empty
    /// string = no callback wired (`GuiInteractable.on_click_callback` stays
    /// `None`) — the image still hit-tests/hovers/presses, it just has
    /// nothing to dispatch on click.
    pub callback_name: Arc<str>,
}

impl GuiImage {
    pub fn new(
        width: f32,
        height: f32,
        tex_key: impl Into<String>,
        offset_x: f32,
        offset_y: f32,
    ) -> Self {
        Self {
            size: Vector2::new(width, height),
            tex_key: tex_key.into(),
            offset: Vector2::new(offset_x, offset_y),
            offset_hover: None,
            offset_pressed: None,
            offset_disabled: None,
            callback_name: Arc::from(""),
        }
    }

    /// Lua-only constructor: sets `callback_name`, dispatched by name through
    /// the Lua-then-Rust callback chain. Rust callers should use `::new` and
    /// pair the entity with a pre-spawned `GuiInteractable::rust(...)`
    /// instead — `callback_name` has no effect once a `GuiInteractable` is
    /// already present (`insert_if_new`).
    #[cfg(feature = "lua")]
    pub fn with_lua_callback(
        width: f32,
        height: f32,
        tex_key: impl Into<String>,
        offset_x: f32,
        offset_y: f32,
        callback_name: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            callback_name: callback_name.into(),
            ..Self::new(width, height, tex_key, offset_x, offset_y)
        }
    }

    pub fn with_offset_hover(mut self, x: f32, y: f32) -> Self {
        self.offset_hover = Some(Vector2::new(x, y));
        self
    }

    pub fn with_offset_pressed(mut self, x: f32, y: f32) -> Self {
        self.offset_pressed = Some(Vector2::new(x, y));
        self
    }

    pub fn with_offset_disabled(mut self, x: f32, y: f32) -> Self {
        self.offset_disabled = Some(Vector2::new(x, y));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guiimage_new() {
        let img = GuiImage::new(32.0, 32.0, "item_sword", 64.0, 32.0);
        assert!((img.size.x - 32.0).abs() < f32::EPSILON);
        assert!((img.size.y - 32.0).abs() < f32::EPSILON);
        assert_eq!(img.tex_key, "item_sword");
        assert!((img.offset.x - 64.0).abs() < f32::EPSILON);
        assert!((img.offset.y - 32.0).abs() < f32::EPSILON);
        assert!(img.callback_name.is_empty());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_guiimage_with_lua_callback() {
        let img =
            GuiImage::with_lua_callback(32.0, 32.0, "item_sword", 64.0, 32.0, "on_sword_clicked");
        assert_eq!(img.tex_key, "item_sword");
        assert!((img.offset.x - 64.0).abs() < f32::EPSILON);
        assert!((img.offset.y - 32.0).abs() < f32::EPSILON);
        assert_eq!(&*img.callback_name, "on_sword_clicked");
    }

    #[test]
    fn test_guiimage_new_has_no_per_state_offsets_by_default() {
        let img = GuiImage::new(32.0, 32.0, "item_sword", 64.0, 32.0);
        assert!(img.offset_hover.is_none());
        assert!(img.offset_pressed.is_none());
        assert!(img.offset_disabled.is_none());
    }

    #[test]
    fn test_guiimage_with_offset_builders_set_per_state_offsets() {
        let img = GuiImage::new(32.0, 32.0, "item_sword", 0.0, 0.0)
            .with_offset_hover(16.0, 0.0)
            .with_offset_pressed(32.0, 0.0)
            .with_offset_disabled(48.0, 0.0);
        assert_eq!(img.offset_hover, Some(Vector2::new(16.0, 0.0)));
        assert_eq!(img.offset_pressed, Some(Vector2::new(32.0, 0.0)));
        assert_eq!(img.offset_disabled, Some(Vector2::new(48.0, 0.0)));
    }
}
