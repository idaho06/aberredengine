//! High-level game state resources.
//!
//! These resources track the authoritative current state of the game and any
//! pending transition requested by systems. See
//! `crate::events::game_state::game_state_changed_observer` for how a
//! transition is applied and hooks are invoked.

use bevy_ecs::prelude::Resource;

/// Discrete high-level states the game can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    None,
    Setup,
    Playing,
    // Paused,
    Quitting,
}

/// Authoritative current game state.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CurrentGameState {
    current: GameState,
}

impl Default for CurrentGameState {
    fn default() -> Self {
        Self::new()
    }
}

impl CurrentGameState {
    /// Create a new state initialized to [`GameState::None`].
    pub fn new() -> Self {
        CurrentGameState {
            current: GameState::None,
        }
    }
    /// Read-only access to the current state.
    pub fn get(&self) -> &GameState {
        &self.current
    }
    /// Update the current state immediately.
    ///
    /// Prefer requesting transitions via [`NextGameState`] and the event
    /// observer when setup/teardown hooks must be triggered.
    pub fn set(&mut self, state: GameState) {
        self.current = state;
    }
}

/// Intent to change to a new game state.
///
/// Use [`NextGameState::set`] to mark a transition as pending; an observer
/// later applies it and clears the request.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NextGameState {
    next: Option<GameState>,
}

impl Default for NextGameState {
    fn default() -> Self {
        Self::new()
    }
}

impl NextGameState {
    /// Create a new value with no transition requested.
    pub fn new() -> Self {
        NextGameState { next: None }
    }

    /// The requested next state, or `None` when no transition is pending.
    pub fn get(&self) -> Option<&GameState> {
        self.next.as_ref()
    }

    /// Request a transition to `next` by marking it as pending.
    ///
    /// An observer will apply the transition and clear the request.
    pub fn set(&mut self, next: GameState) {
        self.next = Some(next);
        // The system `check_pending_state` will handle the state change event emission.
    }

    /// Whether a transition to [`GameState::Quitting`] is requested. A
    /// pending quit wins over a scene request
    /// (`NextScene::switch_due`).
    pub fn quit_requested(&self) -> bool {
        self.next == Some(GameState::Quitting)
    }

    /// Clear the transition request.
    pub fn reset(&mut self) {
        self.next = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_start_idle_and_reset_clears_the_request() {
        assert_eq!(CurrentGameState::default().get(), &GameState::None);

        let mut next = NextGameState::default();
        assert_eq!(next.get(), None);
        next.set(GameState::Playing);
        assert_eq!(next.get(), Some(&GameState::Playing));
        next.reset();
        assert_eq!(next.get(), None);
    }
}
