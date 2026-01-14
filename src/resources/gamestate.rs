//! High-level game state resources.
//!
//! These resources track the authoritative current state of the game and any
//! pending transition requested by systems. See
//! `crate::events::gamestate::observe_gamestate_change_event` for how a
//! transition is applied and hooks are invoked.

use bevy_ecs::prelude::Resource;

#[cfg_attr(not(test), allow(dead_code))]
/// Discrete high-level states the game can be in.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameStates {
    #[default]
    None,
    Setup,
    Playing,
    // Paused,
    Quitting,
}

/// Representation of a requested next state.
///
/// Use [`NextGameState::set`] to mark a transition as pending; an observer
/// will later apply it and reset the value to [`NextGameStates::Unchanged`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum NextGameStates {
    #[default]
    Unchanged,
    Pending(GameStates),
}

/// Authoritative current game state.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct GameState {
    current: GameStates,
}

impl GameState {
    /// Create a new state initialized to [`GameStates::None`].
    pub fn new() -> Self {
        GameState {
            current: GameStates::None,
        }
    }
    /// Read-only access to the current state.
    pub fn get(&self) -> &GameStates {
        &self.current
    }
    /// Update the current state immediately.
    ///
    /// Prefer requesting transitions via [`NextGameState`] and the event
    /// observer when setup/teardown hooks must be triggered.
    pub fn set(&mut self, state: GameStates) {
        self.current = state;
    }
}

/// Intent to change to a new game state.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NextGameState {
    next: NextGameStates,
}

impl NextGameState {
    /// Create a new value initialized to [`NextGameStates::Unchanged`].
    pub fn new() -> Self {
        NextGameState {
            next: NextGameStates::Unchanged,
        }
    }

    /// Get the current transition request.
    pub fn get(&self) -> &NextGameStates {
        &self.next
    }

    /// Request a transition to `next` by marking it as pending.
    ///
    /// An observer will apply the transition and clear the request.
    pub fn set(&mut self, next: GameStates) {
        self.next = NextGameStates::Pending(next);
        // The system `check_pending_state` will handle the state change event emission.
    }

    /// Reset to [`NextGameStates::Unchanged`].
    pub fn reset(&mut self) {
        self.next = NextGameStates::Unchanged;
    }
}
