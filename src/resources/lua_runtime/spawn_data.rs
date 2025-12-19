//! Data structures for entity spawning from Lua.
//!
//! These structs hold component data that Lua scripts specify when spawning entities.
//! They are collected in the `SpawnCmd` struct and processed by Rust systems.

/// Sprite component data for spawning.
#[derive(Debug, Clone, Default)]
pub struct SpriteData {
    pub tex_key: String,
    pub width: f32,
    pub height: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub flip_h: bool,
    pub flip_v: bool,
}

/// BoxCollider component data for spawning.
#[derive(Debug, Clone, Default)]
pub struct ColliderData {
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

/// RigidBody component data for spawning.
#[derive(Debug, Clone, Default)]
pub struct RigidBodyData {
    pub velocity_x: f32,
    pub velocity_y: f32,
}

/// StuckTo component data for spawning.
#[derive(Debug, Clone)]
pub struct StuckToData {
    /// Entity ID (from Entity::to_bits()) of the target to follow
    pub target_entity_id: u64,
    /// Offset from target position
    pub offset_x: f32,
    pub offset_y: f32,
    /// Follow X axis
    pub follow_x: bool,
    /// Follow Y axis
    pub follow_y: bool,
    /// Stored velocity to restore when unstuck
    pub stored_velocity: Option<(f32, f32)>,
}

/// TweenPosition component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenPositionData {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub duration: f32,
    pub easing: String,
    pub loop_mode: String,
}

/// TweenRotation component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenRotationData {
    pub from: f32,
    pub to: f32,
    pub duration: f32,
    pub easing: String,
    pub loop_mode: String,
}

/// TweenScale component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenScaleData {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub duration: f32,
    pub easing: String,
    pub loop_mode: String,
}

/// LuaCollisionRule component data for spawning.
#[derive(Debug, Clone)]
pub struct LuaCollisionRuleData {
    pub group_a: String,
    pub group_b: String,
    pub callback: String,
}

/// Animation component data for spawning.
#[derive(Debug, Clone)]
pub struct AnimationData {
    pub animation_key: String,
}

/// Animation rule data for AnimationController.
#[derive(Debug, Clone)]
pub struct AnimationRuleData {
    pub condition: AnimationConditionData,
    pub set_key: String,
}

/// Condition data for animation rules.
#[derive(Debug, Clone)]
pub enum AnimationConditionData {
    /// Compare a float signal with a value.
    ScalarCmp { key: String, op: String, value: f32 },
    /// Check float signal is in range.
    ScalarRange {
        key: String,
        min: f32,
        max: f32,
        inclusive: bool,
    },
    /// Compare an integer signal with a value.
    IntegerCmp { key: String, op: String, value: i32 },
    /// Check integer signal is in range.
    IntegerRange {
        key: String,
        min: i32,
        max: i32,
        inclusive: bool,
    },
    /// Check that a flag is set.
    HasFlag { key: String },
    /// Check that a flag is not set.
    LacksFlag { key: String },
    /// All nested conditions must pass.
    All(Vec<AnimationConditionData>),
    /// At least one nested condition must pass.
    Any(Vec<AnimationConditionData>),
    /// Negate the nested condition.
    Not(Box<AnimationConditionData>),
}

/// AnimationController component data for spawning.
#[derive(Debug, Clone)]
pub struct AnimationControllerData {
    pub fallback_key: String,
    pub rules: Vec<AnimationRuleData>,
}

/// Phase definition data from Lua
#[derive(Debug, Clone, Default)]
pub struct PhaseData {
    /// Initial phase name
    pub initial: String,
    /// Map of phase name -> callbacks
    pub phases: rustc_hash::FxHashMap<String, PhaseCallbackData>,
}

/// Callback function names for a single phase
#[derive(Debug, Clone, Default)]
pub struct PhaseCallbackData {
    pub on_enter: Option<String>,
    pub on_update: Option<String>,
    pub on_exit: Option<String>,
}

/// Data for dynamic text component
#[derive(Debug, Clone)]
pub struct TextData {
    pub content: String,
    pub font: String,
    pub font_size: f32,
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// RGBA color data (0-255 per channel)
#[derive(Debug, Clone, Copy, Default)]
pub struct ColorData {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Menu action data from Lua.
#[derive(Debug, Clone)]
pub enum MenuActionData {
    SetScene { scene: String },
    ShowSubMenu { menu: String },
    QuitGame,
}

/// Data for spawning a Menu + MenuActions ensemble.
#[derive(Debug, Clone, Default)]
pub struct MenuData {
    /// (id, label)
    pub items: Vec<(String, String)>,
    pub origin_x: f32,
    pub origin_y: f32,
    pub font: String,
    pub font_size: f32,
    pub item_spacing: f32,
    pub use_screen_space: bool,
    pub normal_color: Option<ColorData>,
    pub selected_color: Option<ColorData>,
    pub dynamic_text: Option<bool>,
    /// WorldSignals key for the cursor entity (spawn it first, then reference it here)
    pub cursor_entity_key: Option<String>,
    pub selection_change_sound: Option<String>,
    /// (item_id, action)
    pub actions: Vec<(String, MenuActionData)>,
}

/// Command representing a full entity spawn request from Lua.
/// Contains all optional component data that Lua can specify.
#[derive(Debug, Clone, Default)]
pub struct SpawnCmd {
    /// Group name for the entity
    pub group: Option<String>,
    /// World position (x, y)
    pub position: Option<(f32, f32)>,
    /// Screen position (x, y) - for UI elements
    pub screen_position: Option<(f32, f32)>,
    /// Sprite component data
    pub sprite: Option<SpriteData>,
    /// Dynamic text component data
    pub text: Option<TextData>,
    /// Z-index for render ordering
    pub zindex: Option<i32>,
    /// RigidBody velocity data
    pub rigidbody: Option<RigidBodyData>,
    /// BoxCollider data
    pub collider: Option<ColliderData>,
    /// Whether entity responds to mouse input
    pub mouse_controlled: Option<(bool, bool)>, // (follow_x, follow_y)
    /// Rotation in degrees
    pub rotation: Option<f32>,
    /// Scale (sx, sy)
    pub scale: Option<(f32, f32)>,
    /// Whether entity persists across scene changes
    pub persistent: bool,
    /// Entity signals - scalars
    pub signal_scalars: Vec<(String, f32)>,
    /// Entity signals - integers
    pub signal_integers: Vec<(String, i32)>,
    /// Entity signals - flags
    pub signal_flags: Vec<String>,
    /// Entity signals - strings
    pub signal_strings: Vec<(String, String)>,
    /// Phase data (initial phase + phase definitions)
    pub phase_data: Option<PhaseData>,
    /// Has Signals component (even if empty)
    pub has_signals: bool,
    /// StuckTo component data
    pub stuckto: Option<StuckToData>,
    /// Timer component data (duration, signal)
    pub timer: Option<(f32, String)>,
    /// LuaTimer component data (duration, callback)
    pub lua_timer: Option<(f32, String)>,
    /// SignalBinding component data (key, optional format)
    pub signal_binding: Option<(String, Option<String>)>,
    /// GridLayout component data (path, group, zindex)
    pub grid_layout: Option<(String, String, i32)>,
    /// TweenPosition component data
    pub tween_position: Option<TweenPositionData>,
    /// TweenRotation component data
    pub tween_rotation: Option<TweenRotationData>,
    /// TweenScale component data
    pub tween_scale: Option<TweenScaleData>,
    /// Menu component data (Menu + MenuActions)
    pub menu: Option<MenuData>,
    /// Register spawned entity in WorldSignals with this key
    pub register_as: Option<String>,
    /// LuaCollisionRule component data
    pub lua_collision_rule: Option<LuaCollisionRuleData>,
    /// Animation component data
    pub animation: Option<AnimationData>,
    /// AnimationController component data
    pub animation_controller: Option<AnimationControllerData>,
}
