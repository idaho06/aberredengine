use bevy_ecs::prelude::Resource;

#[derive(Resource, Clone, Copy)]
pub struct WorldTime {
    pub elapsed: f32,
    pub delta: f32,
    pub time_scale: f32,
}

impl Default for WorldTime {
    fn default() -> Self {
        WorldTime {
            elapsed: 0.0,
            delta: 0.0,
            time_scale: 1.0,
        }
    }
}

impl WorldTime {
    /* pub fn delta_seconds(&self) -> f32 {
        self.delta * self.time_scale
    } */
}
