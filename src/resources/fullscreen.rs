//! Full screen toggle resource.
//!
//! The mere presence of this resource indicates that the application should run in
//! full screen mode. Remove it to disable full screen behavior.
//!
use bevy_ecs::prelude::Resource;

/// Marker resource: when present, the application runs in full screen mode.
#[derive(Resource, Clone, Copy)]
pub struct FullScreen {}
