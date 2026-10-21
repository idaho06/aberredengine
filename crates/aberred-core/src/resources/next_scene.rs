//! The pending scene switch request.
//!
//! [`NextScene`] is to scenes what
//! [`NextGameState`] is to game
//! states: a requester writes the target, and the scene switch takes it.
//!
//! Every scene request lands here: Rust `NextScene::set`, a menu's
//! `MenuAction::RequestScene`, a GUI callback's `intents.request_scene` and
//! Lua's `engine.request_scene`. The next scene switch poll applies it.

use bevy_ecs::prelude::Resource;

use crate::resources::gamestate::NextGameState;

/// Request to switch to a scene, by name.
///
/// [`set`](Self::set) records the target; the last request made before the
/// switch takes it wins. [`take`](Self::take) consumes it.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct NextScene(Option<String>);

impl NextScene {
    /// Request a switch to the scene `name`, replacing any earlier request.
    pub fn set(&mut self, name: impl Into<String>) {
        self.0 = Some(name.into());
    }

    /// The requested scene, if any, without consuming it.
    pub fn get(&self) -> Option<&str> {
        self.0.as_deref()
    }

    /// Consume the request: returns the requested scene once, then `None`.
    pub fn take(&mut self) -> Option<String> {
        self.0.take()
    }

    /// Whether a switch is requested.
    pub fn is_pending(&self) -> bool {
        self.0.is_some()
    }

    /// Whether a scene switch poll runs the switch now: a switch is requested
    /// and no quit is pending (a quit wins over a scene request).
    pub fn switch_due(&self, next_state: &NextGameState) -> bool {
        self.is_pending() && !next_state.quit_requested()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::gamestate::GameState;

    #[test]
    fn take_returns_the_requested_scene_once() {
        let mut next = NextScene::default();
        assert!(!next.is_pending());
        assert_eq!(next.get(), None);

        next.set("level01");
        assert!(next.is_pending());
        assert_eq!(next.get(), Some("level01"));

        assert_eq!(next.take().as_deref(), Some("level01"));
        assert!(!next.is_pending());
        assert_eq!(next.take(), None);
    }

    #[test]
    fn the_last_request_wins() {
        let mut next = NextScene::default();
        next.set("a");
        next.set("b");
        assert_eq!(next.take().as_deref(), Some("b"));
    }

    #[test]
    fn a_switch_is_due_for_a_request_unless_a_quit_is_pending() {
        let mut next = NextScene::default();
        let mut state = NextGameState::default();
        assert!(!next.switch_due(&state));

        next.set("b");
        assert!(next.switch_due(&state));

        state.set(GameState::Quitting);
        assert!(!next.switch_due(&state));
    }
}
