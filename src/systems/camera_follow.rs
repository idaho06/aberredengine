//! Camera-follow system.
//!
//! Moves [`Camera2DRes`](crate::resources::camera2d::Camera2DRes) to track
//! the highest-priority entity carrying
//! [`CameraTarget`](crate::components::cameratarget::CameraTarget).
//!
//! Supports four follow modes ([`FollowMode`](crate::resources::camerafollowconfig::FollowMode)):
//! **Instant**, **Lerp** (with configurable easing), **SmoothDamp** (spring-
//! damper), and **Deadzone** (hold-then-catch-up). Optional world-bounds
//! clamping keeps the viewport inside a defined rectangle.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::cameratarget::CameraTarget;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::mapposition::MapPosition;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::{CameraFollowConfig, EasingCurve, FollowMode};
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;

/// Advances the camera toward its target every frame.
///
/// Scheduling: runs after `propagate_transforms` and before `render_system`.
pub fn camera_follow_system(
    targets: Query<(
        Entity,
        &CameraTarget,
        &MapPosition,
        Option<&GlobalTransform2D>,
    )>,
    mut camera: ResMut<Camera2DRes>,
    mut config: ResMut<CameraFollowConfig>,
    time: Res<WorldTime>,
    screensize: Res<ScreenSize>,
) {
    if !config.enabled {
        return;
    }

    // --- 1. Find highest-priority target ---
    let Some((_entity, ct, pos, maybe_gt)) = targets.iter().max_by(|a, b| {
        a.1.priority.cmp(&b.1.priority).then_with(|| b.0.cmp(&a.0)) // lower Entity id wins ties
    }) else {
        return;
    };

    // --- 2. Resolve world position ---
    let target_pos = maybe_gt.map_or(pos.pos, |gt| gt.position);
    let desired = Vector2 {
        x: target_pos.x + config.offset.x,
        y: target_pos.y + config.offset.y,
    };

    let current = camera.0.target;
    let dt = time.delta;

    // --- 3. Apply follow mode ---
    let new_target = match config.mode {
        FollowMode::Instant => desired,

        FollowMode::Lerp => {
            let alpha = lerp_alpha(config.easing, config.lerp_speed, dt);
            vec2_lerp(current, desired, alpha)
        }

        FollowMode::SmoothDamp => {
            let stiffness = config.spring_stiffness;
            let damping = config.spring_damping;

            // Spring force: accelerate toward desired
            config.velocity.x += (desired.x - current.x) * stiffness * dt;
            config.velocity.y += (desired.y - current.y) * stiffness * dt;

            // Damping: bleed off velocity
            let damp = (1.0 - damping * dt).max(0.0);
            config.velocity.x *= damp;
            config.velocity.y *= damp;

            Vector2 {
                x: current.x + config.velocity.x * dt,
                y: current.y + config.velocity.y * dt,
            }
        }

        FollowMode::Deadzone { half_w, half_h } => {
            let dx = desired.x - current.x;
            let dy = desired.y - current.y;

            // Only move on the axis that exceeds the deadzone
            let move_x = if dx.abs() > half_w {
                let overshoot = dx - dx.signum() * half_w;
                current.x + overshoot * (config.lerp_speed * dt).min(1.0)
            } else {
                current.x
            };

            let move_y = if dy.abs() > half_h {
                let overshoot = dy - dy.signum() * half_h;
                current.y + overshoot * (config.lerp_speed * dt).min(1.0)
            } else {
                current.y
            };

            Vector2 {
                x: move_x,
                y: move_y,
            }
        }
    };

    // --- 4. Bounds clamping ---
    let clamped = if let Some(bounds) = config.bounds {
        let zoom = camera.0.zoom.max(f32::EPSILON);
        let half_vw = (screensize.w as f32 / 2.0) / zoom;
        let half_vh = (screensize.h as f32 / 2.0) / zoom;

        Vector2 {
            x: clamp_axis_to_bounds(new_target.x, bounds.x, bounds.width, half_vw),
            y: clamp_axis_to_bounds(new_target.y, bounds.y, bounds.height, half_vh),
        }
    } else {
        new_target
    };

    // --- 5. Commit ---
    camera.0.target = clamped;

    // --- 6. Apply zoom ---
    if (camera.0.zoom - ct.zoom).abs() > 1e-5 {
        let zoom_alpha = lerp_alpha(EasingCurve::EaseOut, config.zoom_lerp_speed, dt);
        camera.0.zoom = lerp_f32(camera.0.zoom, ct.zoom, zoom_alpha).max(f32::EPSILON);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute the interpolation alpha for the current frame.
fn lerp_alpha(easing: EasingCurve, speed: f32, dt: f32) -> f32 {
    match easing {
        // constant-rate: just clamp so we never overshoot
        EasingCurve::Linear => (speed * dt).min(1.0),

        // exponential decay (frame-rate independent)
        EasingCurve::EaseOut => 1.0 - (-speed * dt).exp(),

        // slow start: square the linear alpha
        EasingCurve::EaseIn => {
            let t = (speed * dt).min(1.0);
            t * t
        }

        // smooth both ends: cubic hermite (smoothstep) on the linear alpha
        EasingCurve::EaseInOut => {
            let t = (speed * dt).min(1.0);
            t * t * (3.0 - 2.0 * t)
        }
    }
}

/// Scalar linear interpolation.
fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Clamp a camera axis to bounds without ever inverting the clamp range.
fn clamp_axis_to_bounds(target: f32, origin: f32, size: f32, half_viewport: f32) -> f32 {
    let midpoint = origin + size * 0.5;
    let min = (origin + half_viewport).min(midpoint);
    let max = (origin + size - half_viewport).max(midpoint);
    target.clamp(min, max)
}

/// Component-wise linear interpolation.
fn vec2_lerp(a: Vector2, b: Vector2, t: f32) -> Vector2 {
    Vector2 {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // --- lerp_alpha tests ---

    #[test]
    fn linear_alpha_clamps_to_one() {
        let a = lerp_alpha(EasingCurve::Linear, 100.0, 1.0);
        assert!(approx_eq(a, 1.0));
    }

    #[test]
    fn linear_alpha_proportional() {
        let a = lerp_alpha(EasingCurve::Linear, 5.0, 0.1);
        assert!(approx_eq(a, 0.5));
    }

    #[test]
    fn ease_out_approaches_one() {
        let a = lerp_alpha(EasingCurve::EaseOut, 100.0, 1.0);
        assert!(a > 0.99);
    }

    #[test]
    fn ease_out_small_step() {
        // 1 - exp(-5 * 0.016) ≈ 0.0769
        let a = lerp_alpha(EasingCurve::EaseOut, 5.0, 0.016);
        assert!(a > 0.07 && a < 0.09);
    }

    #[test]
    fn ease_in_is_slower_than_linear() {
        let dt = 0.1;
        let speed = 5.0;
        let linear = lerp_alpha(EasingCurve::Linear, speed, dt);
        let ease_in = lerp_alpha(EasingCurve::EaseIn, speed, dt);
        assert!(ease_in < linear);
    }

    #[test]
    fn ease_in_out_midpoint_near_half() {
        // smoothstep(0.5) = 0.5
        let a = lerp_alpha(EasingCurve::EaseInOut, 5.0, 0.1);
        assert!(approx_eq(a, 0.5));
    }

    // --- vec2_lerp tests ---

    #[test]
    fn lerp_zero_stays() {
        let a = Vector2 { x: 10.0, y: 20.0 };
        let b = Vector2 { x: 50.0, y: 60.0 };
        let r = vec2_lerp(a, b, 0.0);
        assert!(approx_eq(r.x, 10.0));
        assert!(approx_eq(r.y, 20.0));
    }

    #[test]
    fn lerp_one_reaches_target() {
        let a = Vector2 { x: 10.0, y: 20.0 };
        let b = Vector2 { x: 50.0, y: 60.0 };
        let r = vec2_lerp(a, b, 1.0);
        assert!(approx_eq(r.x, 50.0));
        assert!(approx_eq(r.y, 60.0));
    }

    #[test]
    fn lerp_half_is_midpoint() {
        let a = Vector2 { x: 0.0, y: 0.0 };
        let b = Vector2 { x: 100.0, y: 200.0 };
        let r = vec2_lerp(a, b, 0.5);
        assert!(approx_eq(r.x, 50.0));
        assert!(approx_eq(r.y, 100.0));
    }
}
