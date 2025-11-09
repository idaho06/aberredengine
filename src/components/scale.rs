use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Component, Clone, Debug, Copy)]
pub struct Scale {
    pub scale: Vector2,
}
impl Scale {
    pub fn new(sx: f32, sy: f32) -> Self {
        Self {
            scale: Vector2 { x: sx, y: sy },
        }
    }
}
impl Default for Scale {
    fn default() -> Self {
        Self::new(1.0, 1.0)
    }
}
