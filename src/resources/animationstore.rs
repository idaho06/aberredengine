//! Animation resource registry.
//!
//! This module provides a minimal store for animation definitions that can be
//! reused by multiple entities. Systems can look up an animation by a string
//! key and drive playback based on the immutable parameters stored here.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use raylib::prelude::Vector2;
use rustc_hash::FxHashMap;

/// Central registry of reusable animation definitions keyed by string IDs.
#[derive(Resource)]
pub struct AnimationStore {
    pub animations: FxHashMap<String, AnimationResource>,
}

/// Immutable data describing a sprite-sheet or positional animation.
///
/// Fields are intentionally simple to keep the format engine-agnostic. The
/// animation system interprets them to advance frames and compute per-frame
/// positions.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationResource {
    /// Texture key in [`crate::resources::texturestore::TextureStore`].
    pub tex_key: Arc<str>,
    /// Base screen/world position where the animation is anchored.
    pub position: Vector2,
    /// Optional per-frame displacement in the forward axis.
    pub displacement: f32,
    /// Number of frames in the animation.
    pub frame_count: usize,
    /// Frames per second playback speed.
    pub fps: f32,
    /// Whether the animation restarts after the last frame.
    pub looped: bool,
}
