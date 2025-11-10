use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::tween::{Easing, LoopMode, TweenPosition, TweenRotation, TweenScale};
use crate::resources::worldtime::WorldTime;
use bevy_ecs::prelude::*;
use raylib::math::Vector2;

fn ease(e: Easing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match e {
        Easing::Linear => t,
        Easing::QuadIn => t * t,
        Easing::QuadOut => t * (2.0 - t),
        Easing::QuadInOut => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                -1.0 + (4.0 - 2.0 * t) * t
            }
        }
        Easing::CubicIn => t * t * t,
        Easing::CubicOut => {
            let p = t - 1.0;
            p * p * p + 1.0
        }
        Easing::CubicInOut => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                let p = 2.0 * t - 2.0;
                0.5 * p * p * p + 1.0
            }
        } // TODO: sine, elastic, bounce, etc.
    }
}

fn lerp_v2(a: Vector2, b: Vector2, t: f32) -> Vector2 {
    Vector2 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn advance(
    time: &mut f32,
    duration: f32,
    forward: &mut bool,
    playing: &mut bool,
    mode: LoopMode,
    dt: f32,
) {
    let dir = if *forward { 1.0 } else { -1.0 };
    *time += dt * dir;

    let finished_forward = *forward && *time >= duration;
    let finished_backward = !*forward && *time <= 0.0;

    if finished_forward || finished_backward {
        match mode {
            LoopMode::Once => {
                *playing = false;
                *time = time.clamp(0.0, duration);
                // TODO: trigger "finished" event?
            }
            LoopMode::Loop => {
                *time = if finished_forward { 0.0 } else { duration };
            }
            LoopMode::PingPong => {
                *forward = !*forward;
                *time = time.clamp(0.0, duration);
            }
        }
    }
}

pub fn tween_mapposition_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut MapPosition, &mut TweenPosition)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut mp, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_pos = lerp_v2(tw.from, tw.to, t);
        mp.pos = new_pos;
    }
}

pub fn tween_rotation_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut Rotation, &mut TweenRotation)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut rot, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_rot = lerp_f32(tw.from, tw.to, t);
        rot.degrees = new_rot;
    }
}

pub fn tween_scale_system(
    world_time: Res<WorldTime>,
    mut query: Query<(&mut Scale, &mut TweenScale)>,
) {
    let dt = world_time.delta.max(0.0);
    for (mut scale, mut tw) in query.iter_mut() {
        if !tw.playing {
            continue;
        }
        let duration = tw.duration;
        let loop_mode = tw.loop_mode;
        let mut t = tw.time;
        let mut forward = tw.forward;
        let mut playing = tw.playing;
        advance(&mut t, duration, &mut forward, &mut playing, loop_mode, dt);
        tw.time = t;
        tw.forward = forward;
        tw.playing = playing;
        let t = ease(tw.easing, tw.time / duration);
        let new_scale = lerp_v2(tw.from, tw.to, t);
        scale.scale = new_scale;
    }
}
