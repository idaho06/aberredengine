//! Child positioning offset for GUI hierarchies.
//!
//! [`GuiOffset`] is the authored input for a GUI child entity's position
//! relative to its [`ChildOf`](bevy_ecs::hierarchy::ChildOf) parent.
//! `gui_layout_system` resolves it into the child's actual
//! [`ScreenPosition`](super::screenposition::ScreenPosition) every frame —
//! `ChildOf` itself is used for lifecycle only (cascade despawn), not
//! positioning.

use bevy_ecs::prelude::Component;
use crate::math::Vec2;

/// Position offset from a GUI entity's parent, resolved into `ScreenPosition`
/// by `gui_layout_system`.
#[derive(Component, Clone, Copy, Debug)]
pub struct GuiOffset(pub Vec2);
