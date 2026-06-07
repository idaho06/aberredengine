//! Central vocabulary of WorldSignals keys used by the engine itself.
//!
//! Use these constants everywhere a signal key is written or read to get
//! compile-time-checked references and a single rename point.

/// Flag: set by `engine.change_scene(name)` to request a scene transition.
/// The target scene name is stored under [`SCENE`].
pub const SWITCH_SCENE: &str = "switch_scene";

/// Flag: set by `engine.quit()` to request a clean engine shutdown.
pub const QUIT_GAME: &str = "quit_game";

/// String: holds the name of the currently active scene.
pub const SCENE: &str = "scene";

/// Flag: set on an entity's `Signals` component when its non-looped animation
/// reaches the last frame. Cleared when the animation restarts.
pub const ANIMATION_ENDED: &str = "animation_ended";

/// Flag: set on an entity's `Signals` component by `movement` while the entity
/// has non-zero velocity; cleared when stationary. Read by animation rules.
pub const MOVING: &str = "moving";

/// Scalar: squared speed published on an entity's `Signals` component by
/// `movement` each frame. Read by animation rules and exposed to Lua callbacks.
pub const SPEED_SQ: &str = "speed_sq";

/// The scene name used as fallback when `SCENE` has not been set.
pub const DEFAULT_SCENE: &str = "menu";

/// Prefix for integer signals that track live entity counts per group.
/// Full key: `format!("{GROUP_COUNT_PREFIX}{group_name}")`.
pub const GROUP_COUNT_PREFIX: &str = "group_count:";
