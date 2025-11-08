// Counts a number of seconds and then sends an event.
//use crate::events::timer::TimerEvent;
use bevy_ecs::prelude::Component;

#[derive(Component)]
pub struct Timer {
    pub duration: f32,
    pub elapsed: f32,
    pub signal: String,
}
impl Timer {
    pub fn new(duration: f32, signal: impl Into<String>) -> Self {
        Timer {
            duration,
            elapsed: 0.0,
            signal: signal.into(),
        }
    }
    pub fn reset(&mut self) {
        self.elapsed = 0.0;
    }
}
