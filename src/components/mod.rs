//! ECS components for entities.
//!
//! This module groups all component types that can be attached to entities in
//! the game world. Components define data and behaviors such as position,
//! rendering, collision, animation, input control, and more.
//!
//! Submodules overview:
//! - [`animation`] – playback state and a rule-based controller for sprite animations
//! - [`boxcollider`] – axis-aligned rectangular collider for collision detection
//! - [`collision`] – collision callback rules and context for collision observers
//! - [`dynamictext`] – text component for rendering variable strings
//! - [`group`] – tag component for grouping entities by name
//! - [`inputcontrolled`] – input-driven movement intent for keyboard and mouse
//! - [`mapposition`] – world-space position (pivot) for an entity
//! - [`menu`] – interactive menu component and actions
//! - [`persistent`] – marker for entities that persist across scene changes
//! - [`rigidbody`] – simple kinematic body storing velocity
//! - [`rotation`] – rotation angle in degrees
//! - [`scale`] – 2D scale factor for sprites
//! - [`screenposition`] – screen-space position for UI elements
//! - [`signals`] – per-entity signal storage for cross-system communication
//! - [`sprite`] – 2D sprite rendering component
//! - [`timer`] – countdown timer that emits events when finished
//! - [`tween`] – animated interpolation of position, rotation, and scale
//! - [`zindex`] – rendering order hint for 2D drawing

pub mod animation;
pub mod boxcollider;
pub mod collision;
pub mod dynamictext;
pub mod group;
pub mod inputcontrolled;
pub mod mapposition;
pub mod menu;
pub mod persistent;
pub mod rigidbody;
pub mod rotation;
pub mod scale;
pub mod screenposition;
pub mod signals;
pub mod sprite;
pub mod timer;
pub mod tween;
pub mod zindex;
