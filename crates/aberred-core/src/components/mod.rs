//! ECS components for entities.
//!
//! This module groups all component types that can be attached to entities in
//! the game world. Components define data and behaviors such as position,
//! rendering, collision, animation, input control, and more.
//!
//! Submodules overview:
//! - [`animation`] – playback state and a rule-based controller for sprite animations
//! - [`box_collider`] – axis-aligned rectangular collider for collision detection
//! - [`camera_target`] – marks an entity as a candidate for camera following
//! - [`collision`] – group-pair collision rules and contact sides
//! - [`dynamic_text`] – text component for rendering variable strings
//! - [`emitted_particle`] – marker for entities spawned by a particle emitter
//! - [`entity_shader`] – per-entity shader for custom rendering effects
//! - [`grid_layout`] – data-driven grid spawner for tile-based layouts
//! - [`group`] – tag component for grouping entities by name
//! - [`gui_button`] – marker selecting the nine-patch button skin in rendering; hit-test/click state lives in [`gui_interactable`]
//! - [`gui_image`] – clickable image widget (inventory item slots); co-located `Sprite` for the visual, no caption child
//! - [`gui_interactable`] – shared hit-test/click state (size, hover/press/disabled, callbacks) for `GuiButton`/`GuiImage`
//! - [`gui_label`] – static themed GUI label (text caption, no interaction state)
//! - [`gui_offset`] – child positioning offset for GUI hierarchies, resolved by `gui_layout_system`
//! - [`gui_progress_bar`] – themed progress bar (nine-patch track + fill, signal-bound value, four direction variants)
//! - [`gui_window`] – static themed GUI window panel, rendered as a nine-patch background
//! - [`input_controlled`] – input-driven movement intent for keyboard and mouse
//! - [`world_position`] – world-space position (pivot) for an entity
//! - [`menu`] – interactive menu component and actions
//! - [`persistent`] – marker for entities that persist across scene changes
//! - [`phase`] – Rust-based state machine with enter/update/exit function-pointer callbacks
//! - [`position2d`] – generic 2D position component shared by [`world_position`] and [`screen_position`]
//! - [`rigid_body`] – simple kinematic body storing velocity
//! - [`rotation`] – rotation angle in degrees
//! - [`scale`] – 2D scale factor for sprites
//! - [`scene`] – the name carried by each registered scene's persistent entity
//! - [`screen_position`] – screen-space position for UI elements
//! - [`signal_binding`] – binds UI text to signal values for reactive updates
//! - [`signals`] – per-entity signal storage for cross-system communication
//! - [`sprite`] – 2D sprite rendering component
//! - [`stuck_to`] – attaches an entity's position to another entity
//! - [`tilemap`] – tilemap root entity; spawns tile children from a directory path
//! - [`tint`] – color tint for rendering sprites and text
//! - [`tween`] – animated interpolation of position, rotation, and scale
//! - [`z_index`] – rendering order hint for 2D drawing
//!
//! Audio-thread components live in `aberred-audio`; Lua-only ones (`LuaSetup`,
//! `LuaOnTimerFired`, `LuaOnPhaseEntered`, ...) in `aberred-lua`.

pub mod animation;
pub mod box_collider;
pub mod camera_target;
pub mod collision;
pub mod dynamic_text;
pub mod emitted_particle;
pub mod entity_shader;
pub mod global_transform2d;
pub mod grid_layout;
pub mod group;
pub mod gui_button;
pub mod gui_image;
pub mod gui_interactable;
pub mod gui_label;
pub mod gui_offset;
pub mod gui_progress_bar;
pub mod gui_themed;
pub mod gui_window;
pub use gui_themed::Themed;
pub mod input_controlled;
pub mod menu;
pub mod particle_emitter;
pub mod persistent;
pub mod phase;
pub mod position2d;
pub mod rigid_body;
pub mod rotation;
pub mod scale;
pub mod scene;
pub mod screen_position;
pub mod shadow;
pub mod signal_binding;
pub mod signals;
pub mod sprite;
pub mod stuck_to;
pub mod tilemap;
pub mod timer;
pub mod tint;
pub mod ttl;
pub mod tween;
pub mod world_position;
pub mod z_index;
