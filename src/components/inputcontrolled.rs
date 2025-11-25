//! Input-controlled movement components.
//!
//! This module provides components that describe how entities respond to
//! input:
//! - [`InputControlled`] – keyboard-driven directional movement
//! - [`MouseControlled`] – mouse position tracking
//!
//! Systems in [`crate::systems::inputsimplecontroller`] and
//! [`crate::systems::mousecontroller`] read these components to update
//! entity positions or velocities.

use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

/// Movement intent derived from player keyboard input.
///
/// Each field stores the velocity to apply when the corresponding directional
/// input is active. A system should read the current input state and update an
/// entity's velocity or position accordingly.
#[derive(Component, Clone, Copy, Debug)]
///
/// Each field stores the velocity to apply when the corresponding directional
/// input is active. A system should read the current input state and update an
/// entity's velocity or position accordingly.
pub struct InputControlled {
    /// Velocity when moving up.
    pub up_velocity: Vector2,
    /// Velocity when moving down.
    pub down_velocity: Vector2,
    /// Velocity when moving left.
    pub left_velocity: Vector2,
    /// Velocity when moving right.
    pub right_velocity: Vector2,
}

impl InputControlled {
    /// Create a KeyboardControlled component with specified velocities.
    pub fn new(up: Vector2, down: Vector2, left: Vector2, right: Vector2) -> Self {
        Self {
            up_velocity: up,
            down_velocity: down,
            left_velocity: left,
            right_velocity: right,
        }
    }
}
/// Movement controlled by mouse position.
///
/// When attached to an entity, systems will update the entity's position
/// to follow the mouse cursor on the enabled axes.
#[derive(Component, Clone, Copy, Debug)]
pub struct MouseControlled {
    /// Follow mouse X axis.
    pub follow_x: bool,
    /// Follow mouse Y axis.
    pub follow_y: bool,
}

impl MouseControlled {
    /// Create a new MouseControlled component.
    pub fn new(follow_x: bool, follow_y: bool) -> Self {
        Self { follow_x, follow_y }
    }
}

// TODO: MouseDeltaControlled component for relative mouse movement (e.g., for camera control)
