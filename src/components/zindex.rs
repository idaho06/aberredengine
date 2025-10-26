use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// Rendering order hint for 2D drawing.
///
/// Higher values are drawn later (on top). Your renderer can sort by
/// `ZIndex` to achieve a painter's algorithm.
pub struct ZIndex(pub i32);
