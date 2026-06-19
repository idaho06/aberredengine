//! Child positioning offset for GUI hierarchies.
//!
//! [`GuiOffset`] is the authored input for a GUI child entity's position
//! relative to its [`ChildOf`](bevy_ecs::hierarchy::ChildOf) parent.
//! `gui_layout_system` resolves it into the child's actual
//! [`ScreenPosition`](super::screenposition::ScreenPosition) every frame —
//! `ChildOf` itself is used for lifecycle only (cascade despawn), not
//! positioning. See `docs/gui-system-architecture.md`'s Child Layout Model
//! section.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Position offset from a GUI entity's parent, resolved into `ScreenPosition`
/// by `gui_layout_system`.
#[derive(Component, Clone, Copy, Debug)]
pub struct GuiOffset(pub Vector2);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guioffset_construction() {
        let offset = GuiOffset(Vector2::new(20.0, 40.0));
        assert!((offset.0.x - 20.0).abs() < f32::EPSILON);
        assert!((offset.0.y - 40.0).abs() < f32::EPSILON);
    }
}
