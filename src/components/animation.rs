//! Animation components and a simple, data-driven state machine.
//!
//! This module provides two ECS components:
//! - [`Animation`]: tracks which animation is playing and its current frame/time.
//! - [`AnimationController`]: a small rule-based state machine that switches the
//!   current animation key based on game signals/conditions.
//!
//! The intent is to keep the "what to play" and the "when to switch" concerns
//! separate. Systems can update `Animation.elapsed_time` and advance
//! `Animation.frame_index`, while other systems evaluate [`Condition`]s and
//! update `AnimationController.current_key`.
//!
//! Example (pseudo-usage):
//!
//! ```rust,ignore
//! use aberredengine::components::animation::{Animation, AnimationController, Condition, CmpOp};
//!
//! // Attach to an entity
//! let anim = Animation::new("idle");
//! let controller = AnimationController::new("idle")
//!     .with_rule(
//!         Condition::HasFlag { key: "is_running".into() },
//!         "run",
//!     )
//!     .with_rule(
//!         Condition::ScalarCmp { key: "hp".into(), op: CmpOp::Le, value: 0.0 },
//!         "dead",
//!     );
//! ```
#![allow(dead_code, unused_variables)]
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
//use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
/// Per-entity animation playback state.
///
/// Stores a key identifying the active animation, plus the frame cursor and
/// accumulated time used by your animation system to advance frames.
pub struct Animation {
    /// Logical key of the current animation (e.g. "idle", "run").
    pub animation_key: String,
    /// Current frame index within the animation data.
    pub frame_index: usize,
    /// Time in seconds accumulated in the current frame or animation.
    pub elapsed_time: f32,
}
impl Animation {
    /// Create a new [`Animation`] starting from frame 0 and 0 elapsed time.
    ///
    /// The provided key determines which animation your rendering/animation
    /// system will pick from the animation store.
    pub fn new(animation_key: impl Into<String>) -> Self {
        Self {
            animation_key: animation_key.into(),
            frame_index: 0,
            elapsed_time: 0.0,
        }
    }
}

// Animation Controller Component

// Generic, data-driven conditions over Signals
#[derive(Debug, Clone, Serialize, Deserialize)]
/// Comparison operators used in numeric conditions.
pub enum CmpOp {
    /// Less-than
    Lt,
    /// Less-than or equal
    Le,
    /// Greater-than
    Gt,
    /// Greater-than or equal
    Ge,
    /// Equal
    Eq,
    /// Not equal
    Ne,
}

// Condition
#[derive(Debug, Clone, Serialize, Deserialize)]
/// A data-driven predicate evaluated against your runtime "signals"/variables.
///
/// These conditions are intended to be evaluated by a system that has access to
/// your game's signal map (scalars, integers, and flags). Complex expressions
/// can be built using [`Condition::All`], [`Condition::Any`], and
/// [`Condition::Not`].
pub enum Condition {
    /// Compare a float signal with a value using a comparison operator.
    ScalarCmp { key: String, op: CmpOp, value: f32 },
    /// Check whether a float signal lies within a range.
    ScalarRange {
        key: String,
        min: f32,
        max: f32,
        inclusive: bool,
    },
    /// Compare an integer signal with a value using a comparison operator.
    IntegerCmp { key: String, op: CmpOp, value: i32 },
    /// Check whether an integer signal lies within a range.
    IntegerRange {
        key: String,
        min: i32,
        max: i32,
        inclusive: bool,
    },
    /// Check that a boolean/flag signal is present/true.
    HasFlag { key: String },
    /// Check that a boolean/flag signal is absent/false.
    LacksFlag { key: String },
    /// All nested conditions must pass.
    All(Vec<Condition>),
    /// At least one nested condition must pass.
    Any(Vec<Condition>),
    /// Negate the nested condition.
    Not(Box<Condition>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A rule describing when to switch to a target animation key.
pub struct AnimRule {
    /// Predicate to evaluate.
    pub when: Condition,
    /// The animation key to apply when `when` evaluates to true.
    pub set_key: String,
}

//Animation State Machine that defines transitions between animations
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
/// Lightweight animation state machine component.
///
/// The controller holds a `current_key`, a list of transition [`AnimRule`]s,
/// and a `fallback_key` used when no rule matches. A system in your game should
/// evaluate these rules each tick using the latest signals, update
/// `current_key`, and then your animation system can load frames for that key.
pub struct AnimationController {
    /// Current animation key selected by the controller.
    pub current_key: String,
    /// Ordered list of rules; the first matching rule determines the next key.
    pub rules: Vec<AnimRule>,
    /// Default key used when no rules match.
    pub fallback_key: String,
}

impl AnimationController {
    /// Create a controller whose initial and fallback animation is `fallback_key`.
    pub fn new(fallback_key: impl Into<String>) -> Self {
        let fallback_key = fallback_key.into();
        Self {
            current_key: fallback_key.clone(),
            rules: Vec::new(),
            fallback_key,
        }
    }
    /// Append a rule: when `when` is true, set `current_key` to `set_key`.
    ///
    /// Returns `self` to allow fluent chaining.
    pub fn with_rule(mut self, when: Condition, set_key: impl Into<String>) -> Self {
        self.rules.push(AnimRule {
            when,
            set_key: set_key.into(),
        });
        self
    }
}

/*
TODO: Create methods to load/save AnimationController and Animation from/to JSON or other formats
*/
