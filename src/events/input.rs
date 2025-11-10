use bevy_ecs::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    MainDirectionUp,
    MainDirectionDown,
    MainDirectionLeft,
    MainDirectionRight,
    SecondaryDirectionUp,
    SecondaryDirectionDown,
    SecondaryDirectionLeft,
    SecondaryDirectionRight,
    Back,
    Action1,
    Action2,
    Special,
    // ToggleDebug, // Debug toggle has its own event
}

#[derive(Event, Debug, Clone, Copy)]
pub struct InputEvent {
    pub action: InputAction,
    pub pressed: bool,
}
