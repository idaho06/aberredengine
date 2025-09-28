use bevy_ecs::prelude::Resource;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameStates {
    #[default]
    None,
    Setup,
    Playing,
    Paused,
    Quitting,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum NextGameStates {
    #[default]
    Unchanged,
    Pending(GameStates),
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameState {
    current: GameStates,
}

impl GameState {
    pub fn new() -> Self {
        GameState {
            current: GameStates::None,
        }
    }
    pub fn get(&self) -> &GameStates {
        &self.current
    }
    pub fn set(&mut self, state: GameStates) {
        self.current = state;
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NextGameState {
    next: NextGameStates,
}

impl NextGameState {
    pub fn new() -> Self {
        NextGameState {
            next: NextGameStates::Unchanged,
        }
    }

    pub fn get(&self) -> &NextGameStates {
        &self.next
    }

    pub fn set(&mut self, next: GameStates) {
        self.next = NextGameStates::Pending(next);
        // The system `check_pending_state` will handle the state change event emission.
    }

    pub fn reset(&mut self) {
        self.next = NextGameStates::Unchanged;
    }
}
