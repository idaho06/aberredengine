//! Deferred `WorldSignals` writes and scene/quit requests produced by render-side scene
//! callbacks.
//!
//! `GuiCallback` (`aberred_render::resources::scene_table`) runs inside `render_system`,
//! which holds no live `&mut WorldSignals`, [`NextScene`] or [`NextGameState`] — reads come from
//! the snapshot's `SignalSnapshot`, and writes are buffered here instead, applied logic-side by
//! `apply_signal_intents` at the top of the next sim tick.
//!
//! Keep the [`SignalIntent`] variant set minimal; extend on demand as real `GuiCallback` bodies
//! need more `WorldSignals` methods or typed requests.

use crate::resources::gamestate::{GameState, NextGameState};
use crate::resources::next_scene::NextScene;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::{Entity, Resource};

/// One deferred write against [`WorldSignals`], or a scene/quit request, produced by a
/// render-side scene callback and applied logic-side by `apply_signal_intents`.
///
/// Variants are serialized by index into replay files: append new ones at the end.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SignalIntent {
    SetFlag(String),
    RemoveFlag(String),
    SetScalar(String, f32),
    SetInteger(String, i32),
    SetString(String, String),
    RegisterEntity(String, u64),
    /// Request a scene switch: sets [`NextScene`].
    RequestScene(String),
    /// Request a quit: sets [`NextGameState`] to [`GameState::Quitting`].
    RequestQuit,
}

/// Buffer of [`SignalIntent`]s queued during render (`GuiCallback`), drained into
/// [`WorldSignals`], [`NextScene`] and [`NextGameState`] by `apply_signal_intents`.
#[derive(Resource, Debug, Default)]
pub struct SignalIntents(pub Vec<SignalIntent>);

impl SignalIntents {
    // The methods below intentionally mirror `WorldSignals`' own write methods
    // (`set_flag`/`remove_flag`/`set_scalar`/`set_integer`/`set_string`/`register_entity`) by name — when a new
    // `WorldSignals` write method is added, add a matching `SignalIntent` variant + push helper
    // here (and a matching `apply_to` arm below) rather than routing around this buffer.

    /// Queue a flag-set intent.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.0.push(SignalIntent::SetFlag(key.into()));
    }

    /// Queue a flag-remove intent.
    pub fn remove_flag(&mut self, key: impl Into<String>) {
        self.0.push(SignalIntent::RemoveFlag(key.into()));
    }

    /// Queue a scalar-set intent.
    pub fn set_scalar(&mut self, key: impl Into<String>, value: f32) {
        self.0.push(SignalIntent::SetScalar(key.into(), value));
    }

    /// Queue an integer-set intent.
    pub fn set_integer(&mut self, key: impl Into<String>, value: i32) {
        self.0.push(SignalIntent::SetInteger(key.into(), value));
    }

    /// Queue a string-set intent.
    pub fn set_string(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.0
            .push(SignalIntent::SetString(key.into(), value.into()));
    }

    /// Queue an entity-registration intent.
    pub fn register_entity(&mut self, key: impl Into<String>, entity: Entity) {
        self.0
            .push(SignalIntent::RegisterEntity(key.into(), entity.to_bits()));
    }

    /// Queue a scene switch request. Applied to [`NextScene`] at the top of the next sim tick;
    /// the scene switch poll enters the scene later in that same tick.
    ///
    /// During `Setup` the switch only accepts the loading scene and drops any other target. A
    /// deterministic game holds Setup input back to the first `Playing` tick, so there the
    /// request applies after the initial scene is entered.
    pub fn request_scene(&mut self, name: impl Into<String>) {
        self.0.push(SignalIntent::RequestScene(name.into()));
    }

    /// Queue a quit request. Applied to [`NextGameState`] at the top of the next sim tick.
    pub fn request_quit(&mut self) {
        self.0.push(SignalIntent::RequestQuit);
    }

    /// Drain all queued intents into their targets, applying each in the order queued.
    pub fn apply_to(
        &mut self,
        signals: &mut WorldSignals,
        next_scene: &mut NextScene,
        next_state: &mut NextGameState,
    ) {
        for intent in self.0.drain(..) {
            match intent {
                SignalIntent::SetFlag(key) => signals.set_flag(key),
                SignalIntent::RemoveFlag(key) => {
                    signals.remove_flag(&key);
                }
                SignalIntent::SetScalar(key, value) => signals.set_scalar(key, value),
                SignalIntent::SetInteger(key, value) => signals.set_integer(key, value),
                SignalIntent::SetString(key, value) => signals.set_string(key, value),
                SignalIntent::RegisterEntity(key, bits) => {
                    signals.register_entity(key, Entity::from_bits(bits))
                }
                SignalIntent::RequestScene(name) => next_scene.set(name),
                SignalIntent::RequestQuit => next_state.set(GameState::Quitting),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::signal_keys as sk;

    fn targets() -> (WorldSignals, NextScene, NextGameState) {
        (
            WorldSignals::default(),
            NextScene::default(),
            NextGameState::default(),
        )
    }

    #[test]
    fn apply_to_routes_each_variant_to_the_right_domain() {
        let mut intents = SignalIntents::default();
        intents.set_flag("gui:action:save");
        intents.set_scalar("volume", 0.5);
        intents.set_integer("score", 42);
        intents.set_string("player_name", "Ada");

        let (mut signals, mut next_scene, mut next_state) = targets();
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert!(signals.has_flag("gui:action:save"));
        assert_eq!(signals.get_scalar("volume"), Some(0.5));
        assert_eq!(signals.get_integer("score"), Some(42));
        assert_eq!(signals.get_string("player_name"), Some("Ada"));
    }

    #[test]
    fn apply_to_clears_flag() {
        let (mut signals, mut next_scene, mut next_state) = targets();
        signals.set_flag("gui:action:save");

        let mut intents = SignalIntents::default();
        intents.remove_flag("gui:action:save");
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert!(!signals.has_flag("gui:action:save"));
    }

    #[test]
    fn apply_to_routes_register_entity() {
        let mut intents = SignalIntents::default();
        let entity = Entity::from_bits(7);
        intents.register_entity("selected", entity);

        let (mut signals, mut next_scene, mut next_state) = targets();
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert_eq!(signals.get_entity("selected"), Some(entity));
    }

    #[test]
    fn request_scene_sets_next_scene_and_leaves_signals_alone() {
        let mut intents = SignalIntents::default();
        intents.request_scene("b");

        let (mut signals, mut next_scene, mut next_state) = targets();
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert_eq!(next_scene.get(), Some("b"));
        assert_eq!(signals.get_string(sk::SCENE), None);
        assert!(!next_state.quit_requested());
    }

    #[test]
    fn request_quit_sets_next_game_state_to_quitting() {
        let mut intents = SignalIntents::default();
        intents.request_quit();

        let (mut signals, mut next_scene, mut next_state) = targets();
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert!(next_state.quit_requested());
        assert!(!next_scene.is_pending());
    }

    #[test]
    fn apply_to_drains_the_buffer() {
        let mut intents = SignalIntents::default();
        intents.set_flag("a");
        intents.set_flag("b");
        assert_eq!(intents.0.len(), 2);

        let (mut signals, mut next_scene, mut next_state) = targets();
        intents.apply_to(&mut signals, &mut next_scene, &mut next_state);

        assert!(intents.0.is_empty());
    }
}
