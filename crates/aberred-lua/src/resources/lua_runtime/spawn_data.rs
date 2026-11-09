//! Data structures for entity spawning from Lua.
//!
//! These structs hold component data that Lua scripts specify when spawning entities.
//! They are collected in the `SpawnCmd` struct and processed by Rust systems.

use super::lua_tag::LuaTag;
use crate::components::lua_on_collision::LuaOnCollision;
use aberred_core::components::gui_button::GuiButton;
use aberred_core::components::gui_image::GuiImage;
use aberred_core::components::gui_label::GuiLabel;
use aberred_core::components::gui_progress_bar::GuiProgressBar;
use aberred_core::components::gui_window::GuiWindow;
use aberred_core::components::menu::{MenuAction, MenuItem};
use aberred_core::components::shape::{Polygon, Shape, ShapeKind};
use aberred_core::components::timer::TimerMode;
use aberred_core::components::tint::Tint;
use aberred_core::components::tween::{Easing, LoopMode};
use aberred_core::math::Vec2;
use aberred_core::resources::uniform_value::UniformValue;

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

/// Named acceleration force data for spawning.
#[derive(Debug, Clone)]
pub struct ForceData {
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub enabled: bool,
}

/// RigidBody component data for spawning.
#[derive(Debug, Clone, Default)]
pub struct RigidBodyData {
    pub velocity_x: f32,
    pub velocity_y: f32,
    /// Velocity damping factor (0.0 = no friction, higher = more drag).
    pub friction: f32,
    /// Optional maximum speed clamp.
    pub max_speed: Option<f32>,
    /// When true, movement system skips physics calculations.
    pub frozen: bool,
    /// Named acceleration forces to add at spawn time.
    pub forces: Vec<ForceData>,
}

/// Lua phase callback names (`LuaOnPhaseEntered` / `LuaOnPhaseExited` /
/// `LuaOnPhaseUpdate`) for spawning.
#[derive(Debug, Clone, Default)]
pub struct LuaPhaseCallbacksSpawn {
    pub entered: Option<String>,
    pub exited: Option<String>,
    pub update: Option<String>,
}

/// Lua dialogue callback names (`LuaOnDialogueNodeEntered` /
/// `LuaOnDialogueChoiceSelected` / `LuaOnDialogueEnded`) for spawning.
#[derive(Debug, Clone, Default)]
pub struct LuaDialogueCallbacksSpawn {
    pub node_entered: Option<String>,
    pub choice_selected: Option<String>,
    pub ended: Option<String>,
}

/// Lua timer (`Timer` + `LuaOnTimerFired`) data for spawning.
#[derive(Debug, Clone, PartialEq)]
pub struct LuaTimerSpawn {
    /// Seconds before the timer fires
    pub duration: f32,
    /// Lua function to call when the timer fires
    pub callback: String,
    /// Whether the timer repeats or fires once
    pub mode: TimerMode,
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

/// Shared tween configuration (easing, loop mode, duration, direction).
#[derive(Debug, Clone)]
pub struct TweenConfig {
    pub duration: f32,
    pub easing: Easing,
    pub loop_mode: LoopMode,
    pub backwards: bool,
    /// Name of the Lua function to call when the tween finishes. Empty
    /// string means no callback.
    pub callback: String,
}

impl TweenConfig {
    pub fn new(duration: f32) -> Self {
        Self {
            duration,
            easing: Easing::Linear,
            loop_mode: LoopMode::Once,
            backwards: false,
            callback: String::new(),
        }
    }
}

/// Validates a Lua easing name, so a typo raises a Lua error instead of silently
/// becoming `linear` when the tween is built.
pub(crate) fn checked_easing(name: String) -> mlua::Result<Easing> {
    match Easing::from_name(&name) {
        Some(easing) => Ok(easing),
        None => Err(mlua::Error::runtime(format!(
            "Unknown easing '{name}' (expected one of: {})",
            Easing::NAMES.join(", ")
        ))),
    }
}

/// Validates a Lua loop-mode name, so a typo raises a Lua error instead of silently
/// becoming `once` when the tween is built.
pub(crate) fn checked_loop_mode(name: String) -> mlua::Result<LoopMode> {
    match LoopMode::from_name(&name) {
        Some(mode) => Ok(mode),
        None => Err(mlua::Error::runtime(format!(
            "Unknown loop mode '{name}' (expected one of: {})",
            LoopMode::NAMES.join(", ")
        ))),
    }
}

/// Parses a Lua sequence of `{ x = .., y = .. }` tables into points, so a bad entry
/// raises a Lua error naming its 1-based index instead of becoming a stray vertex.
pub(crate) fn parse_points(points: &mlua::Table) -> mlua::Result<Vec<Vec2>> {
    let mut out = Vec::new();
    for (i, value) in points.sequence_values::<mlua::Value>().enumerate() {
        let bad = || {
            mlua::Error::runtime(format!(
                "point {} must be a table with numeric x and y",
                i + 1
            ))
        };
        let mlua::Value::Table(point) = value? else {
            return Err(bad());
        };
        let x: f32 = point.get("x").map_err(|_| bad())?;
        let y: f32 = point.get("y").map_err(|_| bad())?;
        out.push(Vec2::new(x, y));
    }
    Ok(out)
}

/// Parses `points` and triangulates them into a polygon kind, so the sim never
/// triangulates and a bad point raises at the Lua call.
pub(crate) fn polygon_kind(points: &mlua::Table) -> mlua::Result<ShapeKind> {
    Ok(ShapeKind::Polygon(Polygon::new(&parse_points(points)?)))
}

/// Parses a Lua sequence of menu item tables into [`MenuItemData`] values, in
/// sequence order. Each sequence value must be a table with a string `id`, a
/// string `label`, and an `enabled` field that is a strict boolean or absent
/// (defaulting to `true`); anything else is an error that names the offending
/// field (`enabled`, `label`, or `id`).
pub(crate) fn parse_menu_items(table: &mlua::Table) -> mlua::Result<Vec<MenuItemData>> {
    let mut out = Vec::new();
    for (i, value) in table.sequence_values::<mlua::Table>().enumerate() {
        let item = value?;
        let n = i + 1;
        let id = get_menu_item_string(&item, n, "id")?;
        let label = get_menu_item_string(&item, n, "label")?;
        let enabled = match item.get::<mlua::Value>("enabled")? {
            mlua::Value::Nil => true,
            mlua::Value::Boolean(b) => b,
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "menu item #{n}: 'enabled' must be a boolean"
                )));
            }
        };
        out.push(MenuItemData { id, label, enabled });
    }
    Ok(out)
}

/// Reads a required string field from a menu item table, so a missing or
/// non-string field raises a Lua error naming the field and the 1-based item index.
fn get_menu_item_string(item: &mlua::Table, n: usize, field: &str) -> mlua::Result<String> {
    item.get(field)
        .map_err(|_| mlua::Error::runtime(format!("menu item #{n}: '{field}' must be a string")))
}

/// TweenPosition component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenPositionData {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub config: TweenConfig,
}

/// TweenScreenPosition component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenScreenPositionData {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub config: TweenConfig,
}

/// TweenRotation component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenRotationData {
    pub from: f32,
    pub to: f32,
    pub config: TweenConfig,
}

/// TweenScale component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenScaleData {
    pub from_x: f32,
    pub from_y: f32,
    pub to_x: f32,
    pub to_y: f32,
    pub config: TweenConfig,
}

/// TweenTint component data for spawning.
#[derive(Debug, Clone)]
pub struct TweenTintData {
    pub from: Tint,
    pub to: Tint,
    pub config: TweenConfig,
}

/// Lua collision rule data for spawning: a `CollisionRule` plus a
/// `LuaOnCollision` naming its callbacks.
#[derive(Debug, Clone)]
pub struct LuaCollisionRuleData {
    pub group_a: String,
    pub group_b: String,
    pub callbacks: LuaOnCollision,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColorData {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// A single menu item parsed from a Lua `with_menu` items table.
#[derive(Debug, Clone)]
pub struct MenuItemData {
    pub id: String,
    pub label: String,
    /// Whether the item can be selected; disabled items are skipped by navigation.
    pub enabled: bool,
}

impl MenuItemData {
    /// Converts into a core [`MenuItem`], disabled when `enabled` is false.
    pub(crate) fn into_menu_item(self) -> MenuItem {
        let item = MenuItem::new(self.id, self.label);
        if self.enabled { item } else { item.disabled() }
    }
}

/// Data for spawning a Menu + MenuActions ensemble.
#[derive(Debug, Clone, Default)]
pub struct MenuData {
    pub items: Vec<MenuItemData>,
    pub origin_x: f32,
    pub origin_y: f32,
    pub font: String,
    pub font_size: f32,
    pub item_spacing: f32,
    pub use_screen_space: bool,
    pub normal_color: Option<ColorData>,
    pub selected_color: Option<ColorData>,
    /// Explicit RGBA color for disabled items.
    pub disabled_color: Option<ColorData>,
    pub dynamic_text: Option<bool>,
    /// WorldSignals key for the cursor entity (spawn it first, then reference it here)
    pub cursor_entity_key: Option<String>,
    /// Text drawn as the menu's cursor; mutually exclusive with `cursor_entity_key`
    pub cursor_text: Option<String>,
    pub selection_change_sound: Option<String>,
    /// (item_id, action)
    pub actions: Vec<(String, MenuAction)>,
    /// Optional Lua callback invoked when any item is selected.
    pub lua_on_menu_selected: Option<String>,
    /// Optional Lua callback invoked when the highlight moves to another item.
    pub lua_on_menu_selection_changed: Option<String>,
    /// Maximum number of visible items (None = show all, enables scrolling).
    pub visible_count: Option<usize>,
}

/// Shape of the particle emission area.
#[derive(Debug, Clone, Default)]
pub enum ParticleEmitterShapeData {
    /// Emit from a single point.
    #[default]
    Point,
    /// Emit from random positions within a centered rectangle.
    Rect { width: f32, height: f32 },
}

/// TTL configuration for spawned particles.
#[derive(Debug, Clone, Default)]
pub enum ParticleTtlData {
    /// No TTL - particles live until manually despawned.
    #[default]
    None,
    /// Fixed TTL value for all particles.
    Fixed(f32),
    /// Random TTL within a range.
    Range { min: f32, max: f32 },
}

/// Particle emitter component data for spawning.
#[derive(Debug, Clone)]
pub struct ParticleEmitterData {
    /// WorldSignals keys for template entities.
    pub template_keys: Vec<String>,
    /// Emission shape.
    pub shape: ParticleEmitterShapeData,
    /// Offset from owner's position.
    pub offset_x: f32,
    pub offset_y: f32,
    /// Particles spawned per emission event.
    pub particles_per_emission: u32,
    /// Emissions per second.
    pub emissions_per_second: f32,
    /// Remaining emissions before stopping.
    pub emissions_remaining: u32,
    /// Direction arc in degrees (min, max). 0° = up.
    pub arc_min_deg: f32,
    pub arc_max_deg: f32,
    /// Speed range (min, max).
    pub speed_min: f32,
    pub speed_max: f32,
    /// TTL configuration for spawned particles.
    pub ttl: ParticleTtlData,
}

impl Default for ParticleEmitterData {
    fn default() -> Self {
        Self {
            template_keys: Vec::new(),
            shape: ParticleEmitterShapeData::Point,
            offset_x: 0.0,
            offset_y: 0.0,
            particles_per_emission: 1,
            emissions_per_second: 10.0,
            emissions_remaining: 100,
            arc_min_deg: 0.0,
            arc_max_deg: 360.0,
            speed_min: 50.0,
            speed_max: 100.0,
            ttl: ParticleTtlData::None,
        }
    }
}

/// EntityShader component data for spawning.
#[derive(Debug, Clone)]
pub struct EntityShaderData {
    /// Key referencing a shader in the ShaderStore.
    pub key: String,
    /// Uniform values to set on the shader.
    pub uniforms: Vec<(String, UniformValue)>,
}

/// Command representing a full entity spawn request from Lua.
/// Contains all optional component data that Lua can specify.
#[derive(Debug, Clone, Default)]
pub struct SpawnCmd {
    /// Group name for the entity
    pub group: Option<String>,
    /// Rust components to attach (`:with_tag()`), in call order.
    pub tags: Vec<LuaTag>,
    /// World position (x, y)
    pub position: Option<(f32, f32)>,
    /// Screen position (x, y) - for UI elements
    pub screen_position: Option<(f32, f32)>,
    /// Sprite component data
    pub sprite: Option<SpriteData>,
    /// Dynamic text component data
    pub text: Option<TextData>,
    /// Z-index for render ordering
    pub z_index: Option<f32>,
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
    /// Initial phase of a core `Phase`
    pub phase: Option<String>,
    /// Dialogue tree key — spawns the entity with a `DialogueRunner` for the
    /// tree registered under this key in the `DialogueStore` (an unknown key
    /// spawns the entity without a runner)
    pub dialogue: Option<String>,
    /// Lua callbacks for the core `Phase`
    pub lua_phase_callbacks: LuaPhaseCallbacksSpawn,
    /// Lua callbacks for the entity's dialogue events
    pub lua_dialogue_callbacks: LuaDialogueCallbacksSpawn,
    /// Has Signals component (even if empty)
    pub has_signals: bool,
    /// StuckTo component data
    pub stuck_to: Option<StuckToData>,
    /// Lua timer data
    pub lua_timer: Option<LuaTimerSpawn>,
    /// SignalBinding component data (key, optional format)
    pub signal_binding: Option<(String, Option<String>)>,
    /// GridLayout component data (path, group, z_index)
    pub grid_layout: Option<(String, String, f32)>,
    /// TweenPosition component data
    pub tween_position: Option<TweenPositionData>,
    /// TweenScreenPosition component data
    pub tween_screen_position: Option<TweenScreenPositionData>,
    /// TweenRotation component data
    pub tween_rotation: Option<TweenRotationData>,
    /// TweenScale component data
    pub tween_scale: Option<TweenScaleData>,
    /// TweenTint component data
    pub tween_tint: Option<TweenTintData>,
    /// Menu component data (Menu + MenuActions)
    pub menu: Option<MenuData>,
    /// Register spawned entity in WorldSignals with this key
    pub register_as: Option<String>,
    /// Lua collision rule data
    pub lua_collision_rule: Option<LuaCollisionRuleData>,
    /// Animation component data
    pub animation: Option<AnimationData>,
    /// AnimationController component data
    pub animation_controller: Option<AnimationControllerData>,
    /// TTL (time-to-live) in seconds - entity auto-despawns after this duration
    pub ttl: Option<f32>,
    /// Particle emitter component data
    pub particle_emitter: Option<ParticleEmitterData>,
    /// Per-entity shader data
    pub shader: Option<EntityShaderData>,
    /// Color tint (r, g, b, a) for rendering modulation
    pub tint: Option<(u8, u8, u8, u8)>,
    pub shadow: Option<(f32, f32, u8, u8, u8, u8)>,
    /// Parent entity ID (from entity.to_bits()) — inserts ChildOf + GlobalTransform2D on spawn
    pub parent: Option<u64>,
    /// CameraTarget priority (marks entity as candidate for camera following)
    pub camera_target: Option<u8>,
    /// Initial zoom for the CameraTarget (default 1.0 when None)
    pub camera_target_zoom: Option<f32>,
    /// Tilemap path — spawns a tilemap root entity whose tiles become `ChildOf` children
    pub tilemap_path: Option<String>,
    /// GuiWindow component (size, theme_key) — inserted as-is; themed panel
    /// rendered via the named theme looked up in `GuiThemeStore`.
    pub gui_window: Option<GuiWindow>,
    /// Shape component (kind, color, fill, outline color, origin) — inserted as-is
    pub shape: Option<Shape>,
    /// GuiOffset (x, y) — position relative to `parent`, resolved each frame by `gui_layout_system`
    pub gui_offset: Option<(f32, f32)>,
    /// LuaSetup callback name — calls the named Lua function once on `Added<LuaSetup>`
    pub lua_setup: Option<String>,
    /// LuaOnAnimationFinished callback name — called once when the non-looped animation first finishes
    pub lua_on_animation_finished: Option<String>,
    /// LuaOnGuiClicked callback name — called on each `GuiClicked` of the entity's
    /// GuiButton/GuiImage; set by their builders when the name is non-empty
    pub lua_on_gui_clicked: Option<String>,
    /// GuiButton component (size, caption, disabled state) —
    /// inserted as-is; `gui_button_spawn_system` reacts on `Added<GuiButton>`
    /// to spawn the co-located `GuiInteractable` and caption child.
    pub gui_button: Option<GuiButton>,
    /// GuiLabel component (size, caption text) — inserted as-is;
    /// `gui_label_spawn_system` reacts on `Added<GuiLabel>` to spawn the
    /// caption child.
    pub gui_label: Option<GuiLabel>,
    /// GuiImage component (size, texture, per-state offsets) — inserted as-is;
    /// `gui_image_spawn_system` reacts on `Added<GuiImage>` to spawn the
    /// co-located `GuiInteractable` + `Sprite`.
    pub gui_image: Option<GuiImage>,
    /// GuiProgressBar component (size, value, max, direction, theme_key, signal_binding) —
    /// inserted as-is; rendered directly by `render_system` with no spawn system.
    pub gui_progress_bar: Option<GuiProgressBar>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Evaluates `src` (a chunk returning a table) and returns the table together
    /// with the `Lua` that owns it (mlua values panic once the `Lua` is dropped,
    /// so callers must keep it alive while using the table).
    fn lua_table(src: &str) -> (mlua::Lua, mlua::Table) {
        let lua = mlua::Lua::new();
        let table = lua.load(src).eval().expect("chunk returns a table");
        (lua, table)
    }

    #[test]
    fn parse_menu_items_keeps_ids_labels_and_enabled_in_order() {
        let (_lua, table) = lua_table(
            "return {\
                {id='a', label='A'},\
                {id='b', label='B', enabled=false},\
                {id='c', label='C', enabled=true},\
            }",
        );
        let items = parse_menu_items(&table).expect("items parse");
        let parsed: Vec<(&str, &str, bool)> = items
            .iter()
            .map(|item| (item.id.as_str(), item.label.as_str(), item.enabled))
            .collect();
        assert_eq!(
            parsed,
            [("a", "A", true), ("b", "B", false), ("c", "C", true)]
        );
    }

    #[test]
    fn parse_menu_items_rejects_non_boolean_enabled() {
        for src in [
            "return { {id='a', label='A', enabled='no'} }",
            "return { {id='a', label='A', enabled=0} }",
        ] {
            let (_lua, table) = lua_table(src);
            let err = parse_menu_items(&table).expect_err("a non-boolean enabled must error");
            let message = err.to_string();
            assert!(
                message.contains("enabled"),
                "error should name the 'enabled' field, got {message:?}"
            );
        }
    }

    #[test]
    fn parse_menu_items_requires_a_label_and_an_id() {
        for (src, field) in [
            ("return { {id='a'} }", "label"),
            ("return { {label='A'} }", "id"),
        ] {
            let (_lua, table) = lua_table(src);
            let err = parse_menu_items(&table).expect_err("a missing field must error");
            let message = err.to_string();
            assert!(
                message.contains(field),
                "error should name the '{field}' field, got {message:?}"
            );
        }
    }
}
