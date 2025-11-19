//! Shared 2D camera resource.
//!
//! Wraps raylib's [`raylib::prelude::Camera2D`] so that systems can agree on
//! a single world/screen transform. Update this resource to pan/zoom the view.

use bevy_ecs::prelude::Resource;
use raylib::prelude::Camera2D;

/// ECS resource that holds the active 2D camera parameters.
///
/// Typically inserted during setup or scene loading, read by render systems, and optionally
/// mutated by camera-controller systems.
#[derive(Resource)]
pub struct Camera2DRes(pub Camera2D);
