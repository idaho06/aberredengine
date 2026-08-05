//! Per-overlay toggles for the debug mode HUD.
//!
//! When [`DebugMode`](crate::resources::debugmode::DebugMode) is active (F11),
//! individual world-space overlays can be toggled via imgui checkboxes.
use bevy_ecs::prelude::Resource;

/// Controls which world-space debug overlays are rendered.
///
/// All fields default to `true` (everything visible when debug mode is on).
///
/// Inserted independently in both `setup_logic_world` and
/// `setup_render_world` -- two separate instances of this same type, not a
/// `RenderX`-style wrapper mirroring one authoritative copy. That's why it
/// stays in flat `src/resources/` rather than `src/resources/render/`
/// alongside the render-exclusive resources: imgui edits happen render-side,
/// but the render loop diffs and ships changes to the logic side as
/// `LogicMsg::OverlayConfig` (`send_render_mirrors`), rather than the logic
/// side reading a `RenderDebugOverlayConfig` mirror.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct DebugOverlayConfig {
    /// Red AABB outlines around box colliders.
    pub show_collider_boxes: bool,
    /// Green crosshairs at entity positions (MapPosition).
    pub show_position_crosshairs: bool,
    /// Yellow per-entity signal text (flags, scalars, integers).
    pub show_entity_signals: bool,
    /// Orange bounding boxes around DynamicText in world space.
    pub show_text_bounds: bool,
    /// Purple bounding boxes around screen-space sprites.
    pub show_sprite_bounds: bool,
}

impl Default for DebugOverlayConfig {
    fn default() -> Self {
        Self {
            show_collider_boxes: true,
            show_position_crosshairs: true,
            show_entity_signals: true,
            show_text_bounds: true,
            show_sprite_bounds: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_enables_all_overlays() {
        let cfg = DebugOverlayConfig::default();
        assert!(cfg.show_collider_boxes);
        assert!(cfg.show_position_crosshairs);
        assert!(cfg.show_entity_signals);
        assert!(cfg.show_text_bounds);
        assert!(cfg.show_sprite_bounds);
    }
}
