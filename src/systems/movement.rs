//! Movement system with acceleration physics.
//!
//! Integrates entity positions from their current rigid body velocities and
//! the world's unscaled delta time. Supports acceleration-based movement with
//! friction damping and optional speed clamping.

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::signals::Signals;
use crate::events::audio::AudioCmd;
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;

/// Apply acceleration and velocity to `MapPosition` using the frame's delta time.
///
/// This system performs physics integration in the following order:
/// 1. Integrate acceleration into velocity: `velocity += acceleration * delta`
/// 2. Apply friction damping: `velocity *= (1 - friction * delta)`
/// 3. Clamp velocity to max_speed if configured
/// 4. Integrate velocity into position: `position += velocity * delta`
/// 5. Update movement signals for animation/audio systems
pub fn movement(
    mut query: Query<(
        Entity,
        &mut MapPosition,
        &mut RigidBody,
        Option<&mut Signals>,
    )>,
    time: Res<WorldTime>,
    _screensize: Res<ScreenSize>,
    mut _audio_cmd_writer: MessageWriter<AudioCmd>,
) {
    for (_entity, mut position, mut rigidbody, mut maybe_signals) in query.iter_mut() {
        let delta = time.delta;

        // Step 1: Integrate acceleration into velocity
        rigidbody.velocity += rigidbody.acceleration * delta;

        // Step 2: Apply friction damping
        // Using linear damping: velocity *= (1 - friction * delta)
        // This is stable for typical friction values (0-10) and frame rates
        if rigidbody.friction > 0.0 {
            let damping = (1.0 - rigidbody.friction * delta).max(0.0);
            rigidbody.velocity *= damping;

            // Zero out very small velocities to prevent drift
            const VELOCITY_EPSILON: f32 = 0.01;
            if rigidbody.velocity.length() < VELOCITY_EPSILON {
                rigidbody.velocity = Vector2 { x: 0.0, y: 0.0 };
            }
        }

        // Step 3: Clamp velocity to max_speed if configured
        if let Some(max_speed) = rigidbody.max_speed {
            let speed = rigidbody.velocity.length();
            if speed > max_speed {
                rigidbody.velocity = rigidbody.velocity.normalized() * max_speed;
            }
        }

        // Step 4: Integrate velocity into position
        position.pos += rigidbody.velocity * delta;

        // Step 5: Update movement signals
        if let Some(signals) = maybe_signals.as_mut() {
            let speed = rigidbody.velocity.length();
            if speed > 0.0 {
                signals.set_flag("moving");
            } else {
                signals.clear_flag("moving");
            }
            signals.set_scalar("speed", speed);
        }
    }
}
