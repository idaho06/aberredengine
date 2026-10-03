//! Animation resource registry.
//!
//! This module provides a minimal store for animation definitions that can be
//! reused by multiple entities. Systems can look up an animation by a string
//! key and drive playback based on the immutable parameters stored here.

use std::sync::Arc;

use crate::math::Vec2;
use bevy_ecs::prelude::Resource;
use rustc_hash::FxHashMap;

/// Central registry of reusable animation definitions keyed by string IDs.
#[derive(Resource, Default)]
pub struct AnimationStore {
    pub animations: FxHashMap<String, AnimationResource>,
}

impl AnimationStore {
    /// Insert or replace an animation definition with a specific key.
    pub fn insert(&mut self, key: impl Into<String>, animation: AnimationResource) {
        self.animations.insert(key.into(), animation);
    }
}

/// Immutable data describing a sprite-sheet or positional animation.
///
/// Fields are intentionally simple to keep the format engine-agnostic. The
/// animation system interprets them to advance frames and compute per-frame
/// positions.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimationResource {
    /// Texture key in `aberred_render::resources::texturestore::TextureStore`.
    pub tex_key: Arc<str>,
    /// Pixel origin within the texture where frame 0 starts (texture-space, not world/screen).
    pub position: Vec2,
    /// Per-frame horizontal displacement (also the frame width, as frames are packed with no gaps).
    pub horizontal_displacement: f32,
    /// Vertical displacement per row. When non-zero, enables row-wrapping: frames that exceed
    /// the texture width continue on the next row offset by this amount.
    pub vertical_displacement: f32,
    /// Number of frames in the animation.
    pub frame_count: usize,
    /// Frames per second playback speed.
    pub fps: f32,
    /// Whether the animation restarts after the last frame.
    pub looped: bool,
}

impl AnimationResource {
    /// A looping `frame_count`-frame animation of texture `tex_key` at `fps`, with frame 0
    /// at the texture's top-left and frames `frame_width` pixels apart on one row.
    pub fn new(
        tex_key: impl Into<Arc<str>>,
        frame_width: f32,
        frame_count: usize,
        fps: f32,
    ) -> Self {
        Self {
            tex_key: tex_key.into(),
            position: Vec2::ZERO,
            horizontal_displacement: frame_width,
            vertical_displacement: 0.0,
            frame_count,
            fps,
            looped: true,
        }
    }

    /// Sets the pixel position of frame 0 within the texture.
    pub fn with_position(mut self, position: Vec2) -> Self {
        self.position = position;
        self
    }

    /// Sets the row height for row-wrapping: frames past the texture's right edge continue
    /// on the next row, this many pixels down. 0 disables wrapping.
    pub fn with_vertical_displacement(mut self, row_height: f32) -> Self {
        self.vertical_displacement = row_height;
        self
    }

    /// Sets whether the animation restarts after its last frame.
    pub fn with_looped(mut self, looped: bool) -> Self {
        self.looped = looped;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_animation_loops_from_the_texture_origin_without_row_wrap() {
        assert_eq!(
            AnimationResource::new("player", 32.0, 4, 8.0),
            AnimationResource {
                tex_key: "player".into(),
                position: Vec2::ZERO,
                horizontal_displacement: 32.0,
                vertical_displacement: 0.0,
                frame_count: 4,
                fps: 8.0,
                looped: true,
            }
        );
    }
}
