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
//! Explicitly out of scope for this slice: no automatic hover/press/disabled
//! visual feedback (tint/skin swap) — left to the game's Lua callback (e.g.
//! set `Tint` manually from `on_click_callback`). No drag-and-drop; this is
//! click-only.
//!
//! See `docs/gui-system-architecture.md`.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Clickable image slot. `gui_image_spawn_system` reacts on
/// `Added<GuiImage>` to insert the co-located `GuiInteractable` + `Sprite`.
#[derive(Component, Clone, Debug)]
pub struct GuiImage {
    pub size: Vector2,
    pub tex_key: String,
    /// Lua callback name, checked first by the click dispatch chain. Empty
    /// string = no callback wired (`GuiInteractable.on_click_callback` stays
    /// `None`) — the image still hit-tests/hovers/presses, it just has
    /// nothing to dispatch on click.
    pub callback_name: String,
}

impl GuiImage {
    pub fn new(width: f32, height: f32, tex_key: impl Into<String>) -> Self {
        Self {
            size: Vector2::new(width, height),
            tex_key: tex_key.into(),
            callback_name: String::new(),
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
        callback_name: impl Into<String>,
    ) -> Self {
        Self {
            callback_name: callback_name.into(),
            ..Self::new(width, height, tex_key)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guiimage_new() {
        let img = GuiImage::new(32.0, 32.0, "item_sword");
        assert!((img.size.x - 32.0).abs() < f32::EPSILON);
        assert!((img.size.y - 32.0).abs() < f32::EPSILON);
        assert_eq!(img.tex_key, "item_sword");
        assert!(img.callback_name.is_empty());
    }

    #[cfg(feature = "lua")]
    #[test]
    fn test_guiimage_with_lua_callback() {
        let img = GuiImage::with_lua_callback(32.0, 32.0, "item_sword", "on_sword_clicked");
        assert_eq!(img.tex_key, "item_sword");
        assert_eq!(img.callback_name, "on_sword_clicked");
    }
}
