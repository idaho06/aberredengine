//! Z-index component for render ordering.
//!
//! The [`ZIndex`] component provides a simple way to control the drawing
//! order of entities. Entities with higher z-index values are drawn on top
//! of those with lower values.

use bevy_ecs::prelude::Component;

/// Rendering order hint for 2D drawing.
///
/// Higher values are drawn later (on top). Your renderer can sort by
/// `ZIndex` to achieve a painter's algorithm.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
///
/// Higher values are drawn later (on top). Your renderer can sort by
/// `ZIndex` to achieve a painter's algorithm.
pub struct ZIndex(pub i32);
