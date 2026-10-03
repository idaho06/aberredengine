//! [`DeterministicMode`]: present in the logic world of a `.deterministic()`
//! game.

use bevy_ecs::prelude::Resource;

/// Marks a `.deterministic(seed)` game (recording, playing back, or neither).
/// Systems whose behavior differs in deterministic mode check for it with
/// `Option<Res<DeterministicMode>>`.
#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct DeterministicMode;
