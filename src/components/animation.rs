#![allow(dead_code, unused_variables)]
use bevy_ecs::prelude::Component;
use serde::{Deserialize, Serialize};
//use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct Animation {
    pub animation_key: String,
    pub frame_index: usize,
    pub elapsed_time: f32,
}
impl Animation {
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
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

// Condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    ScalarCmp {
        key: String,
        op: CmpOp,
        value: f32,
    },
    ScalarRange {
        key: String,
        min: f32,
        max: f32,
        inclusive: bool,
    },
    IntegerCmp {
        key: String,
        op: CmpOp,
        value: i32,
    },
    IntegerRange {
        key: String,
        min: i32,
        max: i32,
        inclusive: bool,
    },
    HasFlag {
        key: String,
    },
    LacksFlag {
        key: String,
    },
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimRule {
    pub when: Condition,
    pub set_key: String,
}

//Animation State Machine that defines transitions between animations
#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct AnimationController {
    pub current_key: String,
    pub rules: Vec<AnimRule>,
    pub fallback_key: String,
}

impl AnimationController {
    pub fn new(fallback_key: impl Into<String>) -> Self {
        let fallback_key = fallback_key.into();
        Self {
            current_key: fallback_key.clone(),
            rules: Vec::new(),
            fallback_key,
        }
    }
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
