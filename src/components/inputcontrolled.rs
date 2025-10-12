use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Component, Clone, Copy, Debug)]
pub struct InputControlled {
    pub up_velocity: Vector2,
    pub down_velocity: Vector2,
    pub left_velocity: Vector2,
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
