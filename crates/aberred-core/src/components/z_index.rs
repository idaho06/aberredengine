//! Z-index component for render ordering.
//!
//! The [`ZIndex`] component provides a simple way to control the drawing
//! order of entities. Entities with higher z-index values are drawn on top
//! of those with lower values, across sprites and texts in both world and
//! screen space.

use bevy_ecs::prelude::Component;

/// Rendering order hint for 2D drawing.
///
/// Higher values are drawn later (on top). The renderer sorts by
/// `ZIndex` to achieve a painter's algorithm. Sprites and texts share one
/// order within each space, so a text with a lower `ZIndex` than an
/// overlapping sprite draws behind it. At an equal `ZIndex` a text draws over
/// a sprite, and items of the same kind draw in ascending entity order.
#[derive(Component, Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct ZIndex(pub f32);
