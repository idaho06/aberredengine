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
//! - [`emittedparticle`] – marker for entities spawned by a particle emitter
//! - [`entityshader`] – per-entity shader for custom rendering effects
//! - [`gridlayout`] – data-driven grid spawner for tile-based layouts
//! - [`group`] – tag component for grouping entities by name
//! - [`guibutton`] – marker selecting the nine-patch button skin in rendering; hit-test/click state lives in [`guiinteractable`]
//! - [`guiimage`] – clickable image widget (inventory item slots); co-located `Sprite` for the visual, no caption child
//! - [`guiinteractable`] – shared hit-test/click state (size, hover/press/disabled, callbacks) for `GuiButton`/`GuiImage`
//! - [`guilabel`] – static themed GUI label (text caption, no interaction state)
//! - [`guioffset`] – child positioning offset for GUI hierarchies, resolved by `gui_layout_system`
//! - [`guiprogressbar`] – themed progress bar (nine-patch track + fill, signal-bound value, four direction variants)
//! - [`guiwindow`] – static themed GUI window panel, rendered as a nine-patch background
//! - [`inputcontrolled`] – input-driven movement intent for keyboard and mouse
//! - [`mapposition`] – world-space position (pivot) for an entity
//! - [`menu`] – interactive menu component and actions
//! - [`persistent`] – marker for entities that persist across scene changes
//! - [`luaphase`] – *(feature = "lua")* Lua-based state machine with enter/update/exit callbacks
//! - [`luasetup`] – *(feature = "lua")* one-shot entity setup callback fired on `Added<LuaSetup>`
//! - [`phase`] – Rust-based state machine with enter/update/exit function-pointer callbacks
//! - [`position2d`] – generic 2D position component shared by [`mapposition`] and [`screenposition`]
//! - [`rigidbody`] – simple kinematic body storing velocity
//! - [`rotation`] – rotation angle in degrees
//! - [`scale`] – 2D scale factor for sprites
//! - [`screenposition`] – screen-space position for UI elements
//! - [`signalbinding`] – binds UI text to signal values for reactive updates
//! - [`signals`] – per-entity signal storage for cross-system communication
//! - [`sprite`] – 2D sprite rendering component
//! - [`stuckto`] – attaches an entity's position to another entity
//! - [`tilemap`] – tilemap root entity; spawns tile children from a directory path
//! - [`tint`] – color tint for rendering sprites and text
//! - [`luatimer`] – *(feature = "lua")* Lua callback timer for delayed actions
//! - [`tween`] – animated interpolation of position, rotation, and scale
//! - [`zindex`] – rendering order hint for 2D drawing

pub mod animation;
pub mod boxcollider;
pub mod cameratarget;
pub mod collision;
pub mod dynamictext;
pub mod emittedparticle;
pub mod entityshader;
pub mod globaltransform2d;
pub mod gridlayout;
pub mod group;
pub mod guibutton;
pub mod guiimage;
pub mod guiinteractable;
pub mod guilabel;
pub mod guioffset;
pub mod guiprogressbar;
pub mod gui_themed;
pub mod guiwindow;
pub use gui_themed::Themed;
pub mod inputcontrolled;
#[cfg(feature = "lua")]
pub mod lua_on_animation_end;
#[cfg(feature = "lua")]
pub mod lua_on_tween_finished;
#[cfg(feature = "lua")]
pub mod luacollision;
#[cfg(feature = "lua")]
pub mod luaphase;
#[cfg(feature = "lua")]
pub mod luasetup;
#[cfg(feature = "lua")]
pub mod luatimer;
pub mod mapposition;
pub mod menu;
pub mod particleemitter;
pub mod persistent;
pub mod phase;
pub mod position2d;
pub mod rigidbody;
pub mod rotation;
pub mod scale;
pub mod screenposition;
pub mod shadow;
pub mod signalbinding;
pub mod signals;
pub mod sprite;
pub mod stuckto;
pub mod tilemap;
pub mod timer;
pub mod tint;
pub mod ttl;
pub mod tween;
pub mod zindex;
