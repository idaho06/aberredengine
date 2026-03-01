//! ECS components for entities.
//!
//! This module groups all component types that can be attached to entities in
//! the game world. Components define data and behaviors such as position,
//! rendering, collision, animation, input control, and more.
//!
//! Submodules overview:
//! - [`animation`] – playback state and a rule-based controller for sprite animations
//! - [`boxcollider`] – axis-aligned rectangular collider for collision detection
//! - [`cameratarget`] – marks an entity as a candidate for camera following
//! - [`collision`] – collision callback rules and context for collision observers
//! - [`dynamictext`] – text component for rendering variable strings
//! - [`entityshader`] – per-entity shader for custom rendering effects
//! - [`gridlayout`] – data-driven grid spawner for tile-based layouts
//! - [`group`] – tag component for grouping entities by name
//! - [`inputcontrolled`] – input-driven movement intent for keyboard and mouse
//! - [`mapposition`] – world-space position (pivot) for an entity
//! - [`menu`] – interactive menu component and actions
//! - [`persistent`] – marker for entities that persist across scene changes
//! - [`luaphase`] – *(feature = "lua")* Lua-based state machine with enter/update/exit callbacks
//! - [`phase`] – Rust-based state machine with enter/update/exit function-pointer callbacks
//! - [`rigidbody`] – simple kinematic body storing velocity
//! - [`rotation`] – rotation angle in degrees
//! - [`scale`] – 2D scale factor for sprites
//! - [`screenposition`] – screen-space position for UI elements
//! - [`signalbinding`] – binds UI text to signal values for reactive updates
//! - [`signals`] – per-entity signal storage for cross-system communication
//! - [`sprite`] – 2D sprite rendering component
//! - [`stuckto`] – attaches an entity's position to another entity
//! - [`tint`] – color tint for rendering sprites and text
//! - [`luatimer`] – *(feature = "lua")* Lua callback timer for delayed actions
//! - [`tween`] – animated interpolation of position, rotation, and scale
//! - [`zindex`] – rendering order hint for 2D drawing

pub mod animation;
pub mod boxcollider;
pub mod cameratarget;
pub mod globaltransform2d;
pub mod collision;
pub mod dynamictext;
pub mod entityshader;
pub mod gridlayout;
pub mod group;
pub mod inputcontrolled;
#[cfg(feature = "lua")]
pub mod luacollision;
#[cfg(feature = "lua")]
pub mod luaphase;
#[cfg(feature = "lua")]
pub mod luatimer;
pub mod mapposition;
pub mod menu;
pub mod persistent;
pub mod phase;
pub mod rigidbody;
pub mod rotation;
pub mod scale;
pub mod screenposition;
pub mod signalbinding;
pub mod signals;
pub mod sprite;
pub mod stuckto;
pub mod timer;
pub mod tint;
pub mod particleemitter;
pub mod ttl;
pub mod tween;
pub mod zindex;
