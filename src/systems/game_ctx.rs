//! Unified ECS context passed to all Rust game callbacks.
//!
//! [`GameCtx`] is a [`SystemParam`] that bundles every query and resource
//! a game callback is likely to need. It is the single context type shared by
//! timers, phases, collision rules, menus, and scene dispatch.
//!
//! # Usage in callbacks
//!
//! ```ignore
//! fn my_timer(entity: Entity, ctx: &mut GameCtx, input: &InputState) {
//!     if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
//!         rb.velocity = Vector2::zero();
//!     }
//!     ctx.audio.write(AudioCmd::PlayFx { id: "beep".into() });
//!     ctx.world_signals.set_flag("timer_fired");
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::components::timer::TimerCallback`] – fn-pointer type for timer callbacks
//! - [`crate::components::phase::PhaseEnterFn`] etc. – fn-pointer types for phase callbacks
//! - [`crate::components::collision::CollisionCallback`] – fn-pointer type for collision callbacks
//! - [`crate::components::menu::MenuRustCallback`] – fn-pointer type for menu callbacks
//! - [`crate::systems::scene_dispatch::SceneEnterFn`] etc. – fn-pointer types for scene callbacks

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::components::animation::Animation;
use crate::components::boxcollider::BoxCollider;
use crate::components::cameratarget::CameraTarget;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::stuckto::StuckTo;
use crate::events::audio::AudioCmd;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::gameconfig::GameConfig;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::texturestore::TextureStore;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;

/// Unified ECS access passed to all Rust game callbacks.
///
/// Provides commands, a complete set of component queries, and the most
/// commonly needed resources. All Rust callback types —
/// [`TimerCallback`](crate::components::timer::TimerCallback),
/// [`PhaseEnterFn`](crate::components::phase::PhaseEnterFn),
/// [`CollisionCallback`](crate::components::collision::CollisionCallback),
/// [`MenuRustCallback`](crate::components::menu::MenuRustCallback),
/// and the scene callbacks — receive `&mut GameCtx`.
#[derive(SystemParam)]
pub struct GameCtx<'w, 's> {
    /// ECS command buffer for spawning, despawning, inserting/removing components.
    pub commands: Commands<'w, 's>,
    // Mutable queries
    /// Mutable access to entity positions (world-space).
    pub positions: Query<'w, 's, &'static mut MapPosition>,
    /// Mutable access to rigid bodies (velocity, friction, forces).
    pub rigid_bodies: Query<'w, 's, &'static mut RigidBody>,
    /// Mutable access to per-entity signals.
    pub signals: Query<'w, 's, &'static mut Signals>,
    /// Mutable access to animation state.
    pub animations: Query<'w, 's, &'static mut Animation>,
    /// Mutable access to per-entity shaders.
    pub shaders: Query<'w, 's, &'static mut EntityShader>,
    /// Mutable access to camera target markers (priority and zoom).
    pub camera_targets: Query<'w, 's, &'static mut CameraTarget>,
    // Read-only queries
    /// Read-only access to entity groups.
    pub groups: Query<'w, 's, &'static Group>,
    /// Read-only access to screen-space positions.
    pub screen_positions: Query<'w, 's, &'static ScreenPosition>,
    /// Read-only access to box colliders.
    pub box_colliders: Query<'w, 's, &'static BoxCollider>,
    /// Read-only access to world-space transforms (from parent-child hierarchy).
    pub global_transforms: Query<'w, 's, &'static GlobalTransform2D>,
    /// Read-only access to StuckTo relationships.
    pub stuckto: Query<'w, 's, &'static StuckTo>,
    /// Read-only access to rotation.
    pub rotations: Query<'w, 's, &'static Rotation>,
    /// Read-only access to scale.
    pub scales: Query<'w, 's, &'static Scale>,
    /// Read-only access to sprites.
    pub sprites: Query<'w, 's, &'static Sprite>,
    // Resources
    /// Mutable access to global world signals.
    pub world_signals: ResMut<'w, WorldSignals>,
    /// Writer for audio commands (play sounds/music).
    pub audio: MessageWriter<'w, AudioCmd>,
    /// Read-only access to world time (delta, elapsed, time_scale).
    pub world_time: Res<'w, WorldTime>,
    /// Read-only access to loaded textures.
    pub texture_store: Res<'w, TextureStore>,
    /// Read-only access to game configuration (render size, window, FPS, etc.).
    pub config: Res<'w, GameConfig>,
    /// Mutable access to the post-process shader chain and uniforms.
    pub post_process: ResMut<'w, PostProcessShader>,
    /// Mutable access to camera follow configuration (enabled, mode, zoom speed, bounds).
    pub camera_follow: ResMut<'w, CameraFollowConfig>,
}
