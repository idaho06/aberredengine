use bevy_ecs::prelude::Component;

#[derive(Debug, Clone, Component)]
pub struct AnimationComponent {
    pub animation_key: String,
    pub frame_index: usize,
    pub elapsed_time: f32,
}
impl AnimationComponent {
    pub fn new(animation_key: impl Into<String>) -> Self {
        Self {
            animation_key: animation_key.into(),
            frame_index: 0,
            elapsed_time: 0.0,
        }
    }

    /* pub fn update(&mut self, delta_time: f32) {
        self.frame_index =
            ((self.frame_index as f32 + delta_time * self.speed) as usize) % self.frame_count;
    } */
}
