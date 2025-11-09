use bevy_ecs::prelude::Component;
use raylib::prelude::Vector2;

#[derive(Copy, Clone, Debug)]
pub enum LoopMode {
    Once,
    Loop,
    PingPong,
}

#[derive(Copy, Clone, Debug)]
pub enum Easing {
    Linear,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
}

#[derive(Component, Clone, Debug)]
pub struct TweenPosition {
    pub from: Vector2,
    pub to: Vector2,
    pub duration: f32,
    pub easing: Easing,
    pub loop_mode: LoopMode,
    pub playing: bool,
    pub time: f32,
    pub forward: bool,
}

impl TweenPosition {
    pub fn new(from: Vector2, to: Vector2, duration: f32) -> Self {
        TweenPosition {
            from,
            to,
            duration,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
            playing: true,
            time: 0.0,
            forward: true,
        }
    }
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
    pub fn with_loop_mode(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }
    pub fn with_backwards(mut self) -> Self {
        self.time = self.duration;
        self.forward = false;
        self
    }
}

#[derive(Component, Clone, Debug)]
pub struct TweenRotation {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub easing: Easing,
    pub loop_mode: LoopMode,
    pub playing: bool,
    pub time: f32,
    pub forward: bool,
}
impl TweenRotation {
    pub fn new(from: f32, to: f32, duration: f32) -> Self {
        TweenRotation {
            from,
            to,
            duration,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
            playing: true,
            time: 0.0,
            forward: true,
        }
    }
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
    pub fn with_loop_mode(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }
    pub fn with_backwards(mut self) -> Self {
        self.time = self.duration;
        self.forward = false;
        self
    }
}

#[derive(Component, Clone, Debug)]
pub struct TweenScale {
    pub from: Vector2,
    pub to: Vector2,
    pub duration: f32,
    pub easing: Easing,
    pub loop_mode: LoopMode,
    pub playing: bool,
    pub time: f32,
    pub forward: bool,
}

impl TweenScale {
    pub fn new(from: Vector2, to: Vector2, duration: f32) -> Self {
        TweenScale {
            from,
            to,
            duration,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
            playing: true,
            time: 0.0,
            forward: true,
        }
    }
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
    pub fn with_loop_mode(mut self, loop_mode: LoopMode) -> Self {
        self.loop_mode = loop_mode;
        self
    }
    pub fn with_backwards(mut self) -> Self {
        self.time = self.duration;
        self.forward = false;
        self
    }
}
