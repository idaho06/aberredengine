//! Lua scripting runtime resource.
//!
//! This module provides a Lua interpreter that can be used to run game scripts.
//! The runtime exposes an `engine` table with functions that scripts can call
//! to interact with the game engine.
//!
//! # Example
//!
//! ```lua
//! -- From a Lua script
//! engine.log("Hello from Lua!")
//! engine.load_texture("ball", "assets/textures/ball.png")
//! engine.load_font("arcade", "assets/fonts/Arcade.ttf", 128)
//! engine.load_music("menu", "assets/audio/menu.xm")
//! engine.load_sound("ping", "assets/audio/ping.wav")
//!
//! -- Spawning entities
//! engine.spawn()
//!     :with_group("player")
//!     :with_position(400, 700)
//!     :with_sprite("vaus", 48, 12, 24, 6)
//!     :with_zindex(10)
//!     :build()
//! ```

use mlua::prelude::*;
use std::cell::RefCell;

/// Commands that Lua can queue for asset loading.
/// These are processed by Rust systems that have access to the necessary resources.
#[derive(Debug, Clone)]
pub enum AssetCmd {
    /// Load a texture from a file path
    LoadTexture { id: String, path: String },
    /// Load a font from a file path with a specific size
    LoadFont { id: String, path: String, size: i32 },
    /// Load a music track from a file path
    LoadMusic { id: String, path: String },
    /// Load a sound effect from a file path
    LoadSound { id: String, path: String },
    /// Load a tilemap from a directory path
    LoadTilemap { id: String, path: String },
}

// ============================================================================
// Entity Spawning Types
// ============================================================================

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

// ============================================================================
// Audio Commands
// ============================================================================

/// Audio commands that Lua can queue.
#[derive(Debug, Clone)]
pub enum AudioLuaCmd {
    /// Play a music track
    PlayMusic { id: String, looped: bool },
    /// Play a sound effect
    PlaySound { id: String },
    /// Stop all music
    StopAllMusic,
    /// Stop all sounds
    StopAllSounds,
}

// ============================================================================
// World Signal Commands
// ============================================================================

/// Commands to modify WorldSignals from Lua.
#[derive(Debug, Clone)]
pub enum SignalCmd {
    SetScalar { key: String, value: f32 },
    SetInteger { key: String, value: i32 },
    SetString { key: String, value: String },
    SetFlag { key: String },
    ClearFlag { key: String },
}

// ============================================================================
// Phase Commands
// ============================================================================

/// Commands for phase transitions from Lua.
#[derive(Debug, Clone)]
pub enum PhaseCmd {
    /// Request a phase transition for a specific entity
    TransitionTo { entity_id: u64, phase: String },
}

// ============================================================================
// Entity Commands (component manipulation)
// ============================================================================

/// Commands for manipulating entity components from Lua.
#[derive(Debug, Clone)]
pub enum EntityCmd {
    /// Release an entity from StuckTo - removes StuckTo and adds RigidBody with stored velocity
    ReleaseStuckTo { entity_id: u64 },
}

// ============================================================================
// Collision Entity Commands (used inside collision callbacks)
// ============================================================================

/// Commands for manipulating entity components from Lua collision callbacks.
/// These are processed immediately after each collision callback.
#[derive(Debug, Clone)]
pub enum CollisionEntityCmd {
    /// Set entity position
    SetPosition { entity_id: u64, x: f32, y: f32 },
    /// Set entity velocity (RigidBody)
    SetVelocity { entity_id: u64, vx: f32, vy: f32 },
    /// Despawn an entity
    Despawn { entity_id: u64 },
    /// Set an integer signal on an entity's Signals component
    SignalSetInteger {
        entity_id: u64,
        key: String,
        value: i32,
    },
    /// Set a flag on an entity's Signals component
    SignalSetFlag { entity_id: u64, flag: String },
    /// Clear a flag on an entity's Signals component
    SignalClearFlag { entity_id: u64, flag: String },
    /// Insert a Timer component
    InsertTimer {
        entity_id: u64,
        duration: f32,
        signal: String,
    },
    /// Insert a StuckTo component
    InsertStuckTo {
        entity_id: u64,
        target_id: u64,
        follow_x: bool,
        follow_y: bool,
        offset_x: f32,
        offset_y: f32,
        stored_vx: f32,
        stored_vy: f32,
    },
}

// ============================================================================
// Group Commands
// ============================================================================

/// Commands for tracked groups from Lua.
#[derive(Debug, Clone)]
pub enum GroupCmd {
    /// Track a group for entity counting
    TrackGroup { name: String },
    /// Stop tracking a group
    UntrackGroup { name: String },
    /// Clear all tracked groups
    ClearTrackedGroups,
}

// ============================================================================
// Tilemap Commands
// ============================================================================

/// Commands for tilemap operations from Lua.
#[derive(Debug, Clone)]
pub enum TilemapCmd {
    /// Spawn tiles from a loaded tilemap
    SpawnTiles { id: String },
}

// ============================================================================
// Camera Commands
// ============================================================================

/// Commands for camera operations from Lua.
#[derive(Debug, Clone)]
pub enum CameraCmd {
    /// Set the 2D camera with target, offset, rotation and zoom
    SetCamera2D {
        target_x: f32,
        target_y: f32,
        offset_x: f32,
        offset_y: f32,
        rotation: f32,
        zoom: f32,
    },
}

/// Shared state accessible from Lua function closures.
/// This is stored in Lua's app_data and allows Lua functions to queue commands.
struct LuaAppData {
    asset_commands: RefCell<Vec<AssetCmd>>,
    spawn_commands: RefCell<Vec<SpawnCmd>>,
    audio_commands: RefCell<Vec<AudioLuaCmd>>,
    signal_commands: RefCell<Vec<SignalCmd>>,
    phase_commands: RefCell<Vec<PhaseCmd>>,
    entity_commands: RefCell<Vec<EntityCmd>>,
    group_commands: RefCell<Vec<GroupCmd>>,
    tilemap_commands: RefCell<Vec<TilemapCmd>>,
    camera_commands: RefCell<Vec<CameraCmd>>,
    // Collision-scoped command queues (processed immediately after each collision callback)
    collision_entity_commands: RefCell<Vec<CollisionEntityCmd>>,
    collision_signal_commands: RefCell<Vec<SignalCmd>>,
    collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    /// Cached world signal values (read-only snapshot for Lua)
    /// These are updated before calling Lua callbacks
    signal_scalars: RefCell<rustc_hash::FxHashMap<String, f32>>,
    signal_integers: RefCell<rustc_hash::FxHashMap<String, i32>>,
    signal_strings: RefCell<rustc_hash::FxHashMap<String, String>>,
    signal_flags: RefCell<rustc_hash::FxHashSet<String>>,
    group_counts: RefCell<rustc_hash::FxHashMap<String, u32>>,
    /// Cached entity IDs (as u64 from Entity::to_bits())
    signal_entities: RefCell<rustc_hash::FxHashMap<String, u64>>,
    /// Cached tracked group names (read-only snapshot for Lua)
    tracked_groups: RefCell<rustc_hash::FxHashSet<String>>,
}

// ============================================================================
// LuaEntityBuilder - UserData for Lua method chaining
// ============================================================================

/// Entity builder exposed to Lua for fluent entity construction.
///
/// This struct implements `UserData` so Lua can call methods on it using
/// the colon syntax: `engine.spawn():with_position(x, y):build()`
///
/// Each `with_*` method returns `Self` to allow chaining.
/// The `build()` method queues the entity for spawning.
#[derive(Debug, Clone, Default)]
struct LuaEntityBuilder {
    cmd: SpawnCmd,
}

impl LuaEntityBuilder {
    fn new() -> Self {
        Self {
            cmd: SpawnCmd::default(),
        }
    }
}

impl LuaUserData for LuaEntityBuilder {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // :with_group(name) - Set entity group
        methods.add_method_mut("with_group", |_, this, name: String| {
            this.cmd.group = Some(name);
            Ok(this.clone())
        });

        // :with_position(x, y) - Set world position
        methods.add_method_mut("with_position", |_, this, (x, y): (f32, f32)| {
            this.cmd.position = Some((x, y));
            Ok(this.clone())
        });

        // :with_sprite(tex_key, width, height, origin_x, origin_y) - Set sprite
        methods.add_method_mut(
            "with_sprite",
            |_, this, (tex_key, width, height, origin_x, origin_y): (String, f32, f32, f32, f32)| {
                this.cmd.sprite = Some(SpriteData {
                    tex_key,
                    width,
                    height,
                    origin_x,
                    origin_y,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    flip_h: false,
                    flip_v: false,
                });
                Ok(this.clone())
            },
        );

        // :with_sprite_offset(offset_x, offset_y) - Set sprite offset
        methods.add_method_mut(
            "with_sprite_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.offset_x = offset_x;
                    sprite.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_sprite_flip(flip_h, flip_v) - Set sprite flipping
        methods.add_method_mut(
            "with_sprite_flip",
            |_, this, (flip_h, flip_v): (bool, bool)| {
                if let Some(ref mut sprite) = this.cmd.sprite {
                    sprite.flip_h = flip_h;
                    sprite.flip_v = flip_v;
                }
                Ok(this.clone())
            },
        );

        // :with_zindex(z) - Set render order
        methods.add_method_mut("with_zindex", |_, this, z: i32| {
            this.cmd.zindex = Some(z);
            Ok(this.clone())
        });

        // :with_velocity(vx, vy) - Set RigidBody velocity
        methods.add_method_mut("with_velocity", |_, this, (vx, vy): (f32, f32)| {
            this.cmd.rigidbody = Some(RigidBodyData {
                velocity_x: vx,
                velocity_y: vy,
            });
            Ok(this.clone())
        });

        // :with_collider(width, height, origin_x, origin_y) - Set BoxCollider
        methods.add_method_mut(
            "with_collider",
            |_, this, (width, height, origin_x, origin_y): (f32, f32, f32, f32)| {
                this.cmd.collider = Some(ColliderData {
                    width,
                    height,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    origin_x,
                    origin_y,
                });
                Ok(this.clone())
            },
        );

        // :with_collider_offset(offset_x, offset_y) - Set collider offset
        methods.add_method_mut(
            "with_collider_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut collider) = this.cmd.collider {
                    collider.offset_x = offset_x;
                    collider.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_mouse_controlled(follow_x, follow_y) - Enable mouse control
        methods.add_method_mut(
            "with_mouse_controlled",
            |_, this, (follow_x, follow_y): (bool, bool)| {
                this.cmd.mouse_controlled = Some((follow_x, follow_y));
                Ok(this.clone())
            },
        );

        // :with_rotation(degrees) - Set rotation
        methods.add_method_mut("with_rotation", |_, this, degrees: f32| {
            this.cmd.rotation = Some(degrees);
            Ok(this.clone())
        });

        // :with_scale(sx, sy) - Set scale
        methods.add_method_mut("with_scale", |_, this, (sx, sy): (f32, f32)| {
            this.cmd.scale = Some((sx, sy));
            Ok(this.clone())
        });

        // :with_persistent() - Mark entity as persistent across scene changes
        methods.add_method_mut("with_persistent", |_, this, ()| {
            this.cmd.persistent = true;
            Ok(this.clone())
        });

        // :with_signal_scalar(key, value) - Add a scalar signal
        methods.add_method_mut(
            "with_signal_scalar",
            |_, this, (key, value): (String, f32)| {
                this.cmd.signal_scalars.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_signal_integer(key, value) - Add an integer signal
        methods.add_method_mut(
            "with_signal_integer",
            |_, this, (key, value): (String, i32)| {
                this.cmd.signal_integers.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_signal_flag(key) - Add a flag signal
        methods.add_method_mut("with_signal_flag", |_, this, key: String| {
            this.cmd.signal_flags.push(key);
            Ok(this.clone())
        });

        // :with_signal_string(key, value) - Add a string signal
        methods.add_method_mut(
            "with_signal_string",
            |_, this, (key, value): (String, String)| {
                this.cmd.signal_strings.push((key, value));
                Ok(this.clone())
            },
        );

        // :with_screen_position(x, y) - Set screen position (for UI elements)
        methods.add_method_mut("with_screen_position", |_, this, (x, y): (f32, f32)| {
            this.cmd.screen_position = Some((x, y));
            Ok(this.clone())
        });

        // :with_text(content, font, font_size, r, g, b, a) - Set DynamicText
        methods.add_method_mut(
            "with_text",
            |_, this, (content, font, font_size, r, g, b, a): (String, String, f32, u8, u8, u8, u8)| {
                this.cmd.text = Some(TextData {
                    content,
                    font,
                    font_size,
                    r,
                    g,
                    b,
                    a,
                });
                Ok(this.clone())
            },
        );

        // :with_menu(items, origin_x, origin_y, font, font_size, item_spacing, use_screen_space)
        // items is an array-like table of { id = "...", label = "..." }
        methods.add_method_mut(
            "with_menu",
            |_, this,
             (items_table, origin_x, origin_y, font, font_size, item_spacing, use_screen_space): (
                LuaTable,
                f32,
                f32,
                String,
                f32,
                f32,
                bool,
            )| {
                let mut items: Vec<(String, String)> = Vec::new();
                for value in items_table.sequence_values::<LuaTable>() {
                    let item_table = value?;
                    let id: String = item_table.get("id")?;
                    let label: String = item_table.get("label")?;
                    items.push((id, label));
                }

                this.cmd.menu = Some(MenuData {
                    items,
                    origin_x,
                    origin_y,
                    font,
                    font_size,
                    item_spacing,
                    use_screen_space,
                    ..MenuData::default()
                });
                Ok(this.clone())
            },
        );

        // :with_menu_colors(normal_r, normal_g, normal_b, normal_a, selected_r, selected_g, selected_b, selected_a)
        methods.add_method_mut(
            "with_menu_colors",
            |_, this, (nr, ng, nb, na, sr, sg, sb, sa): (u8, u8, u8, u8, u8, u8, u8, u8)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_colors() requires with_menu() first",
                    ));
                };
                menu.normal_color = Some(ColorData {
                    r: nr,
                    g: ng,
                    b: nb,
                    a: na,
                });
                menu.selected_color = Some(ColorData {
                    r: sr,
                    g: sg,
                    b: sb,
                    a: sa,
                });
                Ok(this.clone())
            },
        );

        // :with_menu_dynamic_text(dynamic)
        methods.add_method_mut("with_menu_dynamic_text", |_, this, dynamic: bool| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_dynamic_text() requires with_menu() first",
                ));
            };
            menu.dynamic_text = Some(dynamic);
            Ok(this.clone())
        });

        // :with_menu_cursor(worldsignals_key)
        methods.add_method_mut("with_menu_cursor", |_, this, key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_cursor() requires with_menu() first",
                ));
            };
            menu.cursor_entity_key = Some(key);
            Ok(this.clone())
        });

        // :with_menu_selection_sound(sound_key)
        methods.add_method_mut("with_menu_selection_sound", |_, this, sound_key: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_selection_sound() requires with_menu() first",
                ));
            };
            menu.selection_change_sound = Some(sound_key);
            Ok(this.clone())
        });

        // :with_menu_action_set_scene(item_id, scene)
        methods.add_method_mut(
            "with_menu_action_set_scene",
            |_, this, (item_id, scene): (String, String)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_action_set_scene() requires with_menu() first",
                    ));
                };
                menu.actions
                    .push((item_id, MenuActionData::SetScene { scene }));
                Ok(this.clone())
            },
        );

        // :with_menu_action_show_submenu(item_id, submenu)
        methods.add_method_mut(
            "with_menu_action_show_submenu",
            |_, this, (item_id, submenu): (String, String)| {
                let Some(ref mut menu) = this.cmd.menu else {
                    return Err(LuaError::runtime(
                        "with_menu_action_show_submenu() requires with_menu() first",
                    ));
                };
                menu.actions
                    .push((item_id, MenuActionData::ShowSubMenu { menu: submenu }));
                Ok(this.clone())
            },
        );

        // :with_menu_action_quit(item_id)
        methods.add_method_mut("with_menu_action_quit", |_, this, item_id: String| {
            let Some(ref mut menu) = this.cmd.menu else {
                return Err(LuaError::runtime(
                    "with_menu_action_quit() requires with_menu() first",
                ));
            };
            menu.actions.push((item_id, MenuActionData::QuitGame));
            Ok(this.clone())
        });

        // :with_signals() - Add empty Signals component
        methods.add_method_mut("with_signals", |_, this, ()| {
            this.cmd.has_signals = true;
            Ok(this.clone())
        });

        // :with_phase(table) - Add LuaPhase component with phase definitions
        // Table format: { initial = "phase_name", phases = { phase_name = { on_enter = "fn", on_update = "fn", on_exit = "fn" } } }
        methods.add_method_mut("with_phase", |_, this, table: LuaTable| {
            let initial: String = table.get("initial")?;
            let mut phases = rustc_hash::FxHashMap::default();

            if let Ok(phases_table) = table.get::<LuaTable>("phases") {
                for pair in phases_table.pairs::<String, LuaTable>() {
                    let (phase_name, callbacks_table) = pair?;
                    let callbacks = PhaseCallbackData {
                        on_enter: callbacks_table.get("on_enter").ok(),
                        on_update: callbacks_table.get("on_update").ok(),
                        on_exit: callbacks_table.get("on_exit").ok(),
                    };
                    phases.insert(phase_name, callbacks);
                }
            }

            this.cmd.phase_data = Some(PhaseData { initial, phases });
            Ok(this.clone())
        });

        // :with_stuckto(target_entity_id, follow_x, follow_y) - Attach entity to another entity
        // target_entity_id is obtained from engine.get_entity()
        methods.add_method_mut(
            "with_stuckto",
            |_, this, (target_entity_id, follow_x, follow_y): (u64, bool, bool)| {
                this.cmd.stuckto = Some(StuckToData {
                    target_entity_id,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    follow_x,
                    follow_y,
                    stored_velocity: None,
                });
                Ok(this.clone())
            },
        );

        // :with_stuckto_offset(offset_x, offset_y) - Set offset for StuckTo component
        methods.add_method_mut(
            "with_stuckto_offset",
            |_, this, (offset_x, offset_y): (f32, f32)| {
                if let Some(ref mut stuckto) = this.cmd.stuckto {
                    stuckto.offset_x = offset_x;
                    stuckto.offset_y = offset_y;
                }
                Ok(this.clone())
            },
        );

        // :with_stuckto_stored_velocity(vx, vy) - Set velocity to restore when unstuck
        methods.add_method_mut(
            "with_stuckto_stored_velocity",
            |_, this, (vx, vy): (f32, f32)| {
                if let Some(ref mut stuckto) = this.cmd.stuckto {
                    stuckto.stored_velocity = Some((vx, vy));
                }
                Ok(this.clone())
            },
        );

        // :with_timer(duration, signal) - Add Timer component
        // Timer fires a TimerEvent with the signal after duration seconds
        methods.add_method_mut(
            "with_timer",
            |_, this, (duration, signal): (f32, String)| {
                this.cmd.timer = Some((duration, signal));
                Ok(this.clone())
            },
        );

        // :with_signal_binding(key) - Bind DynamicText to a WorldSignal value
        // The text content will auto-update when the signal changes
        methods.add_method_mut("with_signal_binding", |_, this, key: String| {
            this.cmd.signal_binding = Some((key, None));
            Ok(this.clone())
        });

        // :with_signal_binding_format(format) - Set format string for signal binding
        // Use {} as placeholder, e.g., "Score: {}"
        methods.add_method_mut("with_signal_binding_format", |_, this, format: String| {
            if let Some((key, _)) = this.cmd.signal_binding.take() {
                this.cmd.signal_binding = Some((key, Some(format)));
            }
            Ok(this.clone())
        });

        // :with_grid_layout(path, group, zindex) - Add GridLayout component
        // Spawns entities from a JSON grid layout file
        methods.add_method_mut(
            "with_grid_layout",
            |_, this, (path, group, zindex): (String, String, i32)| {
                this.cmd.grid_layout = Some((path, group, zindex));
                Ok(this.clone())
            },
        );

        // :with_tween_position(from_x, from_y, to_x, to_y, duration) - Add TweenPosition component
        // Animates MapPosition from (from_x, from_y) to (to_x, to_y) over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_position",
            |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
                this.cmd.tween_position = Some(TweenPositionData {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                });
                Ok(this.clone())
            },
        );

        // :with_tween_position_easing(easing) - Set easing for TweenPosition
        // Valid values: "linear", "quad_in", "quad_out", "quad_in_out", "cubic_in", "cubic_out", "cubic_in_out"
        methods.add_method_mut("with_tween_position_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_position_loop(loop_mode) - Set loop mode for TweenPosition
        // Valid values: "once", "loop", "ping_pong"
        methods.add_method_mut("with_tween_position_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_position {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_tween_rotation(from, to, duration) - Add TweenRotation component
        // Animates Rotation from `from` to `to` degrees over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_rotation",
            |_, this, (from, to, duration): (f32, f32, f32)| {
                this.cmd.tween_rotation = Some(TweenRotationData {
                    from,
                    to,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                });
                Ok(this.clone())
            },
        );

        // :with_tween_rotation_easing(easing) - Set easing for TweenRotation
        methods.add_method_mut("with_tween_rotation_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_rotation_loop(loop_mode) - Set loop mode for TweenRotation
        methods.add_method_mut("with_tween_rotation_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_rotation {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_tween_scale(from_x, from_y, to_x, to_y, duration) - Add TweenScale component
        // Animates Scale from (from_x, from_y) to (to_x, to_y) over duration seconds
        // Defaults: easing = "linear", loop_mode = "once"
        methods.add_method_mut(
            "with_tween_scale",
            |_, this, (from_x, from_y, to_x, to_y, duration): (f32, f32, f32, f32, f32)| {
                this.cmd.tween_scale = Some(TweenScaleData {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    duration,
                    easing: "linear".to_string(),
                    loop_mode: "once".to_string(),
                });
                Ok(this.clone())
            },
        );

        // :with_tween_scale_easing(easing) - Set easing for TweenScale
        methods.add_method_mut("with_tween_scale_easing", |_, this, easing: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.easing = easing;
            }
            Ok(this.clone())
        });

        // :with_tween_scale_loop(loop_mode) - Set loop mode for TweenScale
        methods.add_method_mut("with_tween_scale_loop", |_, this, loop_mode: String| {
            if let Some(ref mut tween) = this.cmd.tween_scale {
                tween.loop_mode = loop_mode;
            }
            Ok(this.clone())
        });

        // :with_lua_collision_rule(group_a, group_b, callback) - Add LuaCollisionRule component
        // Registers a collision callback between two entity groups
        // callback is the name of a Lua function to call when collision occurs
        methods.add_method_mut(
            "with_lua_collision_rule",
            |_, this, (group_a, group_b, callback): (String, String, String)| {
                this.cmd.lua_collision_rule = Some(LuaCollisionRuleData {
                    group_a,
                    group_b,
                    callback,
                });
                Ok(this.clone())
            },
        );

        // :register_as(key) - Register spawned entity in WorldSignals with this key
        // This allows Lua to retrieve the entity ID later via engine.get_entity(key)
        methods.add_method_mut("register_as", |_, this, key: String| {
            this.cmd.register_as = Some(key);
            Ok(this.clone())
        });

        // :build() - Queue the entity for spawning
        methods.add_method("build", |lua, this, ()| {
            lua.app_data_ref::<LuaAppData>()
                .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                .spawn_commands
                .borrow_mut()
                .push(this.cmd.clone());
            Ok(())
        });
    }
}

/// Resource holding the Lua interpreter state.
///
/// This is a `NonSend` resource because the Lua state is not thread-safe.
/// It should be initialized once at startup and reused throughout the game.
pub struct LuaRuntime {
    lua: Lua,
}

impl LuaRuntime {
    /// Creates a new Lua runtime and registers the base engine API.
    ///
    /// # Errors
    ///
    /// Returns an error if Lua initialization or API registration fails.
    pub fn new() -> LuaResult<Self> {
        let lua = Lua::new();

        // Set up the package path so `require` can find scripts in assets/scripts/
        lua.load(r#"package.path = "./assets/scripts/?.lua;./assets/scripts/?/init.lua;" .. package.path"#)
            .exec()?;

        // Set up shared app data for Lua closures to access
        lua.set_app_data(LuaAppData {
            asset_commands: RefCell::new(Vec::new()),
            spawn_commands: RefCell::new(Vec::new()),
            audio_commands: RefCell::new(Vec::new()),
            signal_commands: RefCell::new(Vec::new()),
            phase_commands: RefCell::new(Vec::new()),
            entity_commands: RefCell::new(Vec::new()),
            group_commands: RefCell::new(Vec::new()),
            tilemap_commands: RefCell::new(Vec::new()),
            camera_commands: RefCell::new(Vec::new()),
            collision_entity_commands: RefCell::new(Vec::new()),
            collision_signal_commands: RefCell::new(Vec::new()),
            collision_audio_commands: RefCell::new(Vec::new()),
            signal_scalars: RefCell::new(rustc_hash::FxHashMap::default()),
            signal_integers: RefCell::new(rustc_hash::FxHashMap::default()),
            signal_strings: RefCell::new(rustc_hash::FxHashMap::default()),
            signal_flags: RefCell::new(rustc_hash::FxHashSet::default()),
            group_counts: RefCell::new(rustc_hash::FxHashMap::default()),
            signal_entities: RefCell::new(rustc_hash::FxHashMap::default()),
            tracked_groups: RefCell::new(rustc_hash::FxHashSet::default()),
        });

        let runtime = Self { lua };
        runtime.register_base_api()?;
        runtime.register_asset_api()?;
        runtime.register_spawn_api()?;
        runtime.register_audio_api()?;
        runtime.register_signal_api()?;
        runtime.register_phase_api()?;
        runtime.register_entity_api()?;
        runtime.register_group_api()?;
        runtime.register_tilemap_api()?;
        runtime.register_camera_api()?;
        runtime.register_collision_api()?;

        Ok(runtime)
    }

    /// Registers the base `engine` table with logging functions.
    fn register_base_api(&self) -> LuaResult<()> {
        let engine = self.lua.create_table()?;

        // engine.log(message) - General purpose logging
        engine.set(
            "log",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_info(message) - Info level logging
        engine.set(
            "log_info",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua INFO] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_warn(message) - Warning level logging
        engine.set(
            "log_warn",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua WARN] {}", msg);
                Ok(())
            })?,
        )?;

        // engine.log_error(message) - Error level logging
        engine.set(
            "log_error",
            self.lua.create_function(|_, msg: String| {
                eprintln!("[Lua ERROR] {}", msg);
                Ok(())
            })?,
        )?;

        self.lua.globals().set("engine", engine)?;

        Ok(())
    }

    /// Registers asset loading functions in the `engine` table.
    fn register_asset_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.load_texture(id, path) - Queue texture loading
        engine.set(
            "load_texture",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadTexture { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_font(id, path, size) - Queue font loading
        engine.set(
            "load_font",
            self.lua
                .create_function(|lua, (id, path, size): (String, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadFont { id, path, size });
                    Ok(())
                })?,
        )?;

        // engine.load_music(id, path) - Queue music loading
        engine.set(
            "load_music",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadMusic { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_sound(id, path) - Queue sound effect loading
        engine.set(
            "load_sound",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadSound { id, path });
                    Ok(())
                })?,
        )?;

        // engine.load_tilemap(id, path) - Queue tilemap loading
        engine.set(
            "load_tilemap",
            self.lua
                .create_function(|lua, (id, path): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .asset_commands
                        .borrow_mut()
                        .push(AssetCmd::LoadTilemap { id, path });
                    Ok(())
                })?,
        )?;

        Ok(())
    }

    /// Registers entity spawning functions in the `engine` table.
    fn register_spawn_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.spawn() - Create a new entity builder
        engine.set(
            "spawn",
            self.lua
                .create_function(|_, ()| Ok(LuaEntityBuilder::new()))?,
        )?;

        Ok(())
    }

    /// Registers audio functions in the `engine` table.
    fn register_audio_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.play_music(id, looped) - Queue music playback
        engine.set(
            "play_music",
            self.lua
                .create_function(|lua, (id, looped): (String, bool)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .audio_commands
                        .borrow_mut()
                        .push(AudioLuaCmd::PlayMusic { id, looped });
                    Ok(())
                })?,
        )?;

        // engine.play_sound(id) - Queue sound effect playback
        engine.set(
            "play_sound",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::PlaySound { id });
                Ok(())
            })?,
        )?;

        // engine.stop_all_music() - Stop all music
        engine.set(
            "stop_all_music",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::StopAllMusic);
                Ok(())
            })?,
        )?;

        // engine.stop_all_sounds() - Stop all sound effects
        engine.set(
            "stop_all_sounds",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::StopAllSounds);
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers signal read/write functions in the `engine` table.
    fn register_signal_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // ===== READ functions (from cached snapshot) =====

        // engine.get_scalar(key) -> number or nil
        engine.set(
            "get_scalar",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_scalars.borrow().get(&key).copied());
                Ok(value)
            })?,
        )?;

        // engine.get_integer(key) -> integer or nil
        engine.set(
            "get_integer",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_integers.borrow().get(&key).copied());
                Ok(value)
            })?,
        )?;

        // engine.get_string(key) -> string or nil
        engine.set(
            "get_string",
            self.lua.create_function(|lua, key: String| {
                let value = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_strings.borrow().get(&key).cloned());
                Ok(value)
            })?,
        )?;

        // engine.has_flag(key) -> boolean
        engine.set(
            "has_flag",
            self.lua.create_function(|lua, key: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.signal_flags.borrow().contains(&key))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;

        // engine.get_group_count(group) -> integer or nil
        engine.set(
            "get_group_count",
            self.lua.create_function(|lua, group: String| {
                let count = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.group_counts.borrow().get(&group).copied());
                Ok(count)
            })?,
        )?;

        // engine.get_entity(key) -> integer (entity ID) or nil
        // Returns the entity ID as a u64 that can be used with with_stuckto()
        engine.set(
            "get_entity",
            self.lua.create_function(|lua, key: String| {
                let entity_id = lua
                    .app_data_ref::<LuaAppData>()
                    .and_then(|data| data.signal_entities.borrow().get(&key).copied());
                Ok(entity_id)
            })?,
        )?;

        // ===== WRITE functions (queue commands) =====

        // engine.set_scalar(key, value)
        engine.set(
            "set_scalar",
            self.lua
                .create_function(|lua, (key, value): (String, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetScalar { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_integer(key, value)
        engine.set(
            "set_integer",
            self.lua
                .create_function(|lua, (key, value): (String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetInteger { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_string(key, value)
        engine.set(
            "set_string",
            self.lua
                .create_function(|lua, (key, value): (String, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetString { key, value });
                    Ok(())
                })?,
        )?;

        // engine.set_flag(key)
        engine.set(
            "set_flag",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::SetFlag { key });
                Ok(())
            })?,
        )?;

        // engine.clear_flag(key)
        engine.set(
            "clear_flag",
            self.lua.create_function(|lua, key: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearFlag { key });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers phase transition functions in the `engine` table.
    fn register_phase_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.phase_transition(entity_id, phase) - Request phase transition for specific entity
        engine.set(
            "phase_transition",
            self.lua
                .create_function(|lua, (entity_id, phase): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .phase_commands
                        .borrow_mut()
                        .push(PhaseCmd::TransitionTo { entity_id, phase });
                    Ok(())
                })?,
        )?;

        Ok(())
    }

    /// Registers entity manipulation functions in the `engine` table.
    fn register_entity_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.release_stuckto(entity_id) - Release entity from StuckTo, restore velocity
        engine.set(
            "release_stuckto",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .entity_commands
                    .borrow_mut()
                    .push(EntityCmd::ReleaseStuckTo { entity_id });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers tracked group functions in the `engine` table.
    fn register_group_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.track_group(name) - Register a group for entity counting
        engine.set(
            "track_group",
            self.lua.create_function(|lua, name: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::TrackGroup { name });
                Ok(())
            })?,
        )?;

        // engine.untrack_group(name) - Stop tracking a group
        engine.set(
            "untrack_group",
            self.lua.create_function(|lua, name: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::UntrackGroup { name });
                Ok(())
            })?,
        )?;

        // engine.clear_tracked_groups() - Clear all tracked groups
        engine.set(
            "clear_tracked_groups",
            self.lua.create_function(|lua, ()| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .group_commands
                    .borrow_mut()
                    .push(GroupCmd::ClearTrackedGroups);
                Ok(())
            })?,
        )?;

        // engine.has_tracked_group(name) -> boolean
        // Check if a group is being tracked (reads from cached data)
        engine.set(
            "has_tracked_group",
            self.lua.create_function(|lua, name: String| {
                let has = lua
                    .app_data_ref::<LuaAppData>()
                    .map(|data| data.tracked_groups.borrow().contains(&name))
                    .unwrap_or(false);
                Ok(has)
            })?,
        )?;

        Ok(())
    }

    /// Registers tilemap functions in the `engine` table.
    fn register_tilemap_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.spawn_tiles(id) - Queue tile spawning from a loaded tilemap
        engine.set(
            "spawn_tiles",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .tilemap_commands
                    .borrow_mut()
                    .push(TilemapCmd::SpawnTiles { id });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Registers camera functions in the `engine` table.
    fn register_camera_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.set_camera(target_x, target_y, offset_x, offset_y, rotation, zoom)
        // Set the 2D camera parameters
        engine.set(
            "set_camera",
            self.lua.create_function(
                |lua,
                 (target_x, target_y, offset_x, offset_y, rotation, zoom): (
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                )| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .camera_commands
                        .borrow_mut()
                        .push(CameraCmd::SetCamera2D {
                            target_x,
                            target_y,
                            offset_x,
                            offset_y,
                            rotation,
                            zoom,
                        });
                    Ok(())
                },
            )?,
        )?;

        Ok(())
    }

    fn register_collision_api(&self) -> LuaResult<()> {
        let engine: LuaTable = self.lua.globals().get("engine")?;

        // engine.entity_set_position(entity_id, x, y)
        // Sets the position of an entity during collision handling
        engine.set(
            "entity_set_position",
            self.lua
                .create_function(|lua, (entity_id, x, y): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SetPosition { entity_id, x, y });
                    Ok(())
                })?,
        )?;

        // engine.entity_set_velocity(entity_id, vx, vy)
        // Sets the velocity of an entity during collision handling
        engine.set(
            "entity_set_velocity",
            self.lua
                .create_function(|lua, (entity_id, vx, vy): (u64, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SetVelocity { entity_id, vx, vy });
                    Ok(())
                })?,
        )?;

        // engine.entity_despawn(entity_id)
        // Despawns an entity during collision handling
        engine.set(
            "entity_despawn",
            self.lua.create_function(|lua, entity_id: u64| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_entity_commands
                    .borrow_mut()
                    .push(CollisionEntityCmd::Despawn { entity_id });
                Ok(())
            })?,
        )?;

        // engine.entity_signal_set_integer(entity_id, key, value)
        // Sets an integer signal on an entity during collision handling
        engine.set(
            "entity_signal_set_integer",
            self.lua
                .create_function(|lua, (entity_id, key, value): (u64, String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalSetInteger {
                            entity_id,
                            key,
                            value,
                        });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_set_flag(entity_id, flag)
        // Sets a flag signal on an entity during collision handling
        engine.set(
            "entity_signal_set_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalSetFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_signal_clear_flag(entity_id, flag)
        // Clears a flag signal on an entity during collision handling
        engine.set(
            "entity_signal_clear_flag",
            self.lua
                .create_function(|lua, (entity_id, flag): (u64, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::SignalClearFlag { entity_id, flag });
                    Ok(())
                })?,
        )?;

        // engine.entity_insert_timer(entity_id, duration, signal)
        // Inserts a timer component on an entity during collision handling
        engine.set(
            "entity_insert_timer",
            self.lua.create_function(
                |lua, (entity_id, duration, signal): (u64, f32, String)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::InsertTimer {
                            entity_id,
                            duration,
                            signal,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.entity_insert_stuckto(entity_id, target_id, follow_x, follow_y, offset_x, offset_y, stored_vx, stored_vy)
        // Inserts a StuckTo component on an entity during collision handling
        engine.set(
            "entity_insert_stuckto",
            self.lua.create_function(
                |lua,
                 (
                    entity_id,
                    target_id,
                    follow_x,
                    follow_y,
                    offset_x,
                    offset_y,
                    stored_vx,
                    stored_vy,
                ): (u64, u64, bool, bool, f32, f32, f32, f32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_entity_commands
                        .borrow_mut()
                        .push(CollisionEntityCmd::InsertStuckTo {
                            entity_id,
                            target_id,
                            follow_x,
                            follow_y,
                            offset_x,
                            offset_y,
                            stored_vx,
                            stored_vy,
                        });
                    Ok(())
                },
            )?,
        )?;

        // engine.collision_play_sound(sound_name)
        // Plays a sound effect during collision handling
        engine.set(
            "collision_play_sound",
            self.lua.create_function(|lua, id: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_audio_commands
                    .borrow_mut()
                    .push(AudioLuaCmd::PlaySound { id });
                Ok(())
            })?,
        )?;

        // engine.collision_set_integer(key, value)
        // Sets a global integer signal during collision handling
        engine.set(
            "collision_set_integer",
            self.lua
                .create_function(|lua, (key, value): (String, i32)| {
                    lua.app_data_ref::<LuaAppData>()
                        .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                        .collision_signal_commands
                        .borrow_mut()
                        .push(SignalCmd::SetInteger { key, value });
                    Ok(())
                })?,
        )?;

        // engine.collision_set_flag(flag)
        // Sets a global flag signal during collision handling
        engine.set(
            "collision_set_flag",
            self.lua.create_function(|lua, flag: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::SetFlag { key: flag });
                Ok(())
            })?,
        )?;

        // engine.collision_clear_flag(flag)
        // Clears a global flag signal during collision handling
        engine.set(
            "collision_clear_flag",
            self.lua.create_function(|lua, flag: String| {
                lua.app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?
                    .collision_signal_commands
                    .borrow_mut()
                    .push(SignalCmd::ClearFlag { key: flag });
                Ok(())
            })?,
        )?;

        Ok(())
    }

    /// Drains all queued asset commands.
    ///
    /// Call this from a Rust system after Lua has queued commands via
    /// `engine.load_texture()`, etc. The system can then process them
    /// with access to the necessary resources (RaylibHandle, etc.).
    pub fn drain_asset_commands(&self) -> Vec<AssetCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.asset_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued spawn commands.
    ///
    /// Call this from a Rust system after Lua has queued entity spawns via
    /// `engine.spawn():...:build()`. The system can then process them
    /// with access to ECS Commands.
    pub fn drain_spawn_commands(&self) -> Vec<SpawnCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.spawn_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued audio commands.
    pub fn drain_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.audio_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued signal commands.
    pub fn drain_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.signal_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued phase commands.
    pub fn drain_phase_commands(&self) -> Vec<PhaseCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.phase_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued entity commands.
    pub fn drain_entity_commands(&self) -> Vec<EntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.entity_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued group commands.
    pub fn drain_group_commands(&self) -> Vec<GroupCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.group_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued tilemap commands.
    pub fn drain_tilemap_commands(&self) -> Vec<TilemapCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.tilemap_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued camera commands.
    pub fn drain_camera_commands(&self) -> Vec<CameraCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| data.camera_commands.borrow_mut().drain(..).collect())
            .unwrap_or_default()
    }

    /// Drains all queued collision entity commands.
    /// Call this after processing Lua collision callbacks to apply entity changes.
    pub fn drain_collision_entity_commands(&self) -> Vec<CollisionEntityCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_entity_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision signal commands.
    /// Call this after processing Lua collision callbacks to apply signal changes.
    pub fn drain_collision_signal_commands(&self) -> Vec<SignalCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_signal_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Drains all queued collision audio commands.
    /// Call this after processing Lua collision callbacks to play sounds.
    pub fn drain_collision_audio_commands(&self) -> Vec<AudioLuaCmd> {
        self.lua
            .app_data_ref::<LuaAppData>()
            .map(|data| {
                data.collision_audio_commands
                    .borrow_mut()
                    .drain(..)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Updates the cached world signal values that Lua can read.
    /// Call this before invoking Lua callbacks so they have fresh data.
    pub fn update_signal_cache(
        &self,
        scalars: &rustc_hash::FxHashMap<String, f32>,
        integers: &rustc_hash::FxHashMap<String, i32>,
        strings: &rustc_hash::FxHashMap<String, String>,
        flags: &rustc_hash::FxHashSet<String>,
        group_counts: &rustc_hash::FxHashMap<String, u32>,
        entities: &rustc_hash::FxHashMap<String, u64>,
    ) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.signal_scalars.borrow_mut() = scalars.clone();
            *data.signal_integers.borrow_mut() = integers.clone();
            *data.signal_strings.borrow_mut() = strings.clone();
            *data.signal_flags.borrow_mut() = flags.clone();
            *data.group_counts.borrow_mut() = group_counts.clone();
            *data.signal_entities.borrow_mut() = entities.clone();
        }
    }

    /// Updates the cached tracked groups that Lua can read.
    /// Call this before invoking Lua callbacks so they have fresh data.
    pub fn update_tracked_groups_cache(&self, groups: &rustc_hash::FxHashSet<String>) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            *data.tracked_groups.borrow_mut() = groups.clone();
        }
    }

    /// Loads and executes a Lua script from a file path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Lua script file
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the script has syntax/runtime errors.
    pub fn run_script(&self, path: &str) -> LuaResult<()> {
        let script = std::fs::read_to_string(path)
            .map_err(|e| LuaError::ExternalError(std::sync::Arc::new(e)))?;
        self.lua.load(&script).set_name(path).exec()
    }

    /// Calls a global Lua function by name with the given arguments.
    ///
    /// # Type Parameters
    ///
    /// * `A` - Argument types (must implement `IntoLuaMulti`)
    /// * `R` - Return type (must implement `FromLuaMulti`)
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the global function to call
    /// * `args` - Arguments to pass to the function
    ///
    /// # Errors
    ///
    /// Returns an error if the function doesn't exist or execution fails.
    pub fn call_function<A, R>(&self, name: &str, args: A) -> LuaResult<R>
    where
        A: IntoLuaMulti,
        R: FromLuaMulti,
    {
        let func: LuaFunction = self.lua.globals().get(name)?;
        func.call(args)
    }

    /// Checks if a global function exists.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the function to check
    pub fn has_function(&self, name: &str) -> bool {
        self.lua.globals().get::<LuaFunction>(name).is_ok()
    }

    /// Returns a reference to the underlying Lua state.
    ///
    /// Use this for advanced operations like registering custom userdata types.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

impl Default for LuaRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create Lua runtime")
    }
}
