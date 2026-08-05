//! Debug toggle resource.
//!
//! The mere presence of this resource indicates that debug rendering and
//! diagnostics should be enabled. Remove it to disable debug behavior.

use bevy_ecs::prelude::Resource;

/// Marker resource: when present, systems may draw overlays or print extra logs.
#[derive(Resource, Clone, Copy)]
pub struct DebugMode {}
