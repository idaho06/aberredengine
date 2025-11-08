// Timer event that is emitted when the timer reaches count end.
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;

#[derive(Event, Debug, Clone, PartialEq, Eq)]
pub struct TimerEvent {
    pub entity: Entity,
    pub signal: String,
}
