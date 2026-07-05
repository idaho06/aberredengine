//! Deferred `WorldSignals` writes produced by render-side scene callbacks.
//!
//! [`GuiCallback`](crate::systems::scene_dispatch::GuiCallback) runs inside `render_system`
//! (Phase 5d, `docs/render-simulation-separation-brainstorm.md`), which no longer holds a live
//! `&mut WorldSignals` — reads come from the snapshot's `SignalSnapshot`, and writes are
//! buffered here instead, applied logic-side by `apply_signal_intents` at the top of the next
//! FIXED substep (Phase 6d; was "the next frame's VARIABLE schedule" pre-6d).
//!
//! Keep the [`SignalIntent`] variant set minimal; extend on demand as real `GuiCallback` bodies
//! need more `WorldSignals` methods.

use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::Resource;

/// One deferred write against [`WorldSignals`], produced by a render-side scene callback and
/// applied logic-side by `apply_signal_intents`.
#[derive(Debug, Clone, PartialEq)]
pub enum SignalIntent {
    SetFlag(String),
    ClearFlag(String),
    SetScalar(String, f32),
    SetInteger(String, i32),
    SetString(String, String),
}

/// Buffer of [`SignalIntent`]s queued during render (`GuiCallback`), drained into
/// [`WorldSignals`] by `apply_signal_intents`.
#[derive(Resource, Debug, Default)]
pub struct SignalIntents(pub Vec<SignalIntent>);

impl SignalIntents {
    // The methods below intentionally mirror `WorldSignals`' own setter names
    // (`set_flag`/`clear_flag`/`set_scalar`/`set_integer`/`set_string`) one-for-one — when a new
    // `WorldSignals` write method is added, add a matching `SignalIntent` variant + push helper
    // here (and a matching `apply_to` arm below) rather than routing around this buffer.

    /// Queue a flag-set intent.
    pub fn set_flag(&mut self, key: impl Into<String>) {
        self.0.push(SignalIntent::SetFlag(key.into()));
    }

    /// Queue a flag-clear intent.
    pub fn clear_flag(&mut self, key: impl Into<String>) {
        self.0.push(SignalIntent::ClearFlag(key.into()));
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

    /// Drain all queued intents into `signals`, applying each in the order queued.
    pub fn apply_to(&mut self, signals: &mut WorldSignals) {
        for intent in self.0.drain(..) {
            match intent {
                SignalIntent::SetFlag(key) => signals.set_flag(key),
                SignalIntent::ClearFlag(key) => signals.clear_flag(&key),
                SignalIntent::SetScalar(key, value) => signals.set_scalar(key, value),
                SignalIntent::SetInteger(key, value) => signals.set_integer(key, value),
                SignalIntent::SetString(key, value) => signals.set_string(key, value),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_to_routes_each_variant_to_the_right_domain() {
        let mut intents = SignalIntents::default();
        intents.set_flag("gui:action:save");
        intents.set_scalar("volume", 0.5);
        intents.set_integer("score", 42);
        intents.set_string("player_name", "Ada");

        let mut signals = WorldSignals::default();
        intents.apply_to(&mut signals);

        assert!(signals.has_flag("gui:action:save"));
        assert_eq!(signals.get_scalar("volume"), Some(0.5));
        assert_eq!(signals.get_integer("score"), Some(42));
        assert_eq!(
            signals.get_string("player_name").map(String::as_str),
            Some("Ada")
        );
    }

    #[test]
    fn apply_to_clears_flag() {
        let mut signals = WorldSignals::default();
        signals.set_flag("gui:action:save");

        let mut intents = SignalIntents::default();
        intents.clear_flag("gui:action:save");
        intents.apply_to(&mut signals);

        assert!(!signals.has_flag("gui:action:save"));
    }

    #[test]
    fn apply_to_drains_the_buffer() {
        let mut intents = SignalIntents::default();
        intents.set_flag("a");
        intents.set_flag("b");
        assert_eq!(intents.0.len(), 2);

        let mut signals = WorldSignals::default();
        intents.apply_to(&mut signals);

        assert!(intents.0.is_empty());
    }
}
