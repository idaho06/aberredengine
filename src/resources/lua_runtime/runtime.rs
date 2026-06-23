//! Lua runtime core implementation.
//!
//! This module contains the `LuaRuntime` struct which manages the Lua interpreter
//! and provides the `engine` table API to Lua scripts.

use super::commands::*;
use super::input_snapshot::InputSnapshot;
use super::spawn_data::*;
use crate::resources::worldsignals::SignalSnapshot;
use mlua::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cell::RefCell;
use std::sync::Arc;

/// Cached camera state snapshot for Lua to read via `engine.get_camera()` / `engine.get_camera_view_rect()`.
///
/// Updated before calling Lua callbacks via `update_camera_cache()`.
/// Only populated during `lua_plugin::update`; during `on_setup` and `on_switch_scene`
/// the snapshot holds `Default` values (zoom=1.0, everything else 0).
pub(super) struct CameraSnapshot {
    pub target_x: f32,
    pub target_y: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub rotation: f32,
    pub zoom: f32,
    pub view_x: f32,
    pub view_y: f32,
    pub view_w: f32,
    pub view_h: f32,
}

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            target_x: 0.0,
            target_y: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
            rotation: 0.0,
            zoom: 1.0,
            view_x: 0.0,
            view_y: 0.0,
            view_w: 0.0,
            view_h: 0.0,
        }
    }
}

/// Cached game configuration snapshot for Lua to read.
pub(super) struct GameConfigSnapshot {
    pub fullscreen: bool,
    pub vsync: bool,
    pub target_fps: u32,
    pub render_width: u32,
    pub render_height: u32,
    pub background_r: u8,
    pub background_g: u8,
    pub background_b: u8,
    pub pixel_snap_camera: bool,
}

impl Default for GameConfigSnapshot {
    fn default() -> Self {
        Self {
            fullscreen: false,
            vsync: false,
            target_fps: 60,
            render_width: 640,
            render_height: 360,
            background_r: 80,
            background_g: 80,
            background_b: 80,
            pixel_snap_camera: true,
        }
    }
}

/// Shared state accessible from Lua function closures.
/// This is stored in Lua's app_data and allows Lua functions to queue commands.
///
/// Queue fields must stay in sync with the list in `queue_registry.rs`.
/// Drain methods and the body of `clear_all_commands` are generated from that list.
#[derive(Default)]
pub(super) struct LuaAppData {
    // Command queues — keep in sync with queue_registry.rs lua_queues! list
    pub(super) asset_commands: RefCell<Vec<AssetCmd>>,
    pub(super) spawn_commands: RefCell<Vec<SpawnCmd>>,
    pub(super) audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) signal_commands: RefCell<Vec<SignalCmd>>,
    pub(super) phase_commands: RefCell<Vec<PhaseCmd>>,
    pub(super) entity_commands: RefCell<Vec<EntityCmd>>,
    pub(super) group_commands: RefCell<Vec<GroupCmd>>,
    pub(super) camera_commands: RefCell<Vec<CameraCmd>>,
    pub(super) animation_commands: RefCell<Vec<AnimationCmd>>,
    pub(super) render_commands: RefCell<Vec<RenderCmd>>,
    pub(super) gui_theme_commands: RefCell<Vec<RenderCmd>>,
    pub(super) clone_commands: RefCell<Vec<CloneCmd>>,
    pub(super) gameconfig_commands: RefCell<Vec<GameConfigCmd>>,
    pub(super) camera_follow_commands: RefCell<Vec<CameraFollowCmd>>,
    pub(super) input_commands: RefCell<Vec<InputCmd>>,
    pub(super) map_commands: RefCell<Vec<MapLuaCmd>>,
    pub(super) collision_entity_commands: RefCell<Vec<EntityCmd>>,
    pub(super) collision_signal_commands: RefCell<Vec<SignalCmd>>,
    pub(super) collision_audio_commands: RefCell<Vec<AudioLuaCmd>>,
    pub(super) collision_spawn_commands: RefCell<Vec<SpawnCmd>>,
    pub(super) collision_clone_commands: RefCell<Vec<CloneCmd>>,
    pub(super) collision_phase_commands: RefCell<Vec<PhaseCmd>>,
    pub(super) collision_camera_commands: RefCell<Vec<CameraCmd>>,
    // Read-only caches — updated before each Lua callback
    pub(super) signal_snapshot: RefCell<Arc<SignalSnapshot>>,
    pub(super) tracked_groups: RefCell<FxHashSet<String>>,
    pub(super) gameconfig_snapshot: RefCell<GameConfigSnapshot>,
    pub(super) bindings_snapshot: RefCell<std::collections::HashMap<String, String>>,
    pub(super) camera_snapshot: RefCell<CameraSnapshot>,
    /// Resolved Lua function handles, cached by global name. Cleared on
    /// scene switch via `clear_function_cache` (see `get_function_cached`).
    pub(super) function_cache: RefCell<FxHashMap<String, LuaFunction>>,
    /// Frame number and snapshot last written to the pooled input table, used
    /// by `update_input_table` to skip redundant writes within a frame and
    /// diff against the previous frame's values.
    pub(super) last_input: RefCell<Option<(u64, InputSnapshot)>>,
}

/// Pooled inner tables for one entity's `signals` ctx field
/// (`flags`/`integers`/`scalars`/`strings`), reused in place via
/// `mlua::Table::clear()` across callbacks. See [`populate_entity_signals`](super::context::populate_entity_signals).
#[derive(Clone)]
pub struct SignalsCtxTables {
    pub flags: LuaTable,
    pub integers: LuaTable,
    pub scalars: LuaTable,
    pub strings: LuaTable,
    /// Scratch buffer reused by `clear_map_table` to collect keys without
    /// allocating a fresh `Vec` on every call. Always empty between calls.
    pub scratch_keys: RefCell<Vec<LuaValue>>,
}

impl SignalsCtxTables {
    fn create(lua: &Lua) -> LuaResult<Self> {
        Ok(Self {
            flags: lua.create_table()?,
            integers: lua.create_table()?,
            scalars: lua.create_table()?,
            strings: lua.create_table()?,
            scratch_keys: RefCell::new(Vec::new()),
        })
    }
}

/// Pooled collision context tables, owned directly by `LuaRuntime` and reused for
/// every collision via cheap `Clone` (each field is a ref-counted `LuaTable` handle).
#[derive(Clone)]
pub struct CollisionCtxTables {
    pub ctx: LuaTable,
    pub entity_a: LuaTable,
    pub entity_b: LuaTable,
    pub pos_a: LuaTable,
    pub pos_b: LuaTable,
    pub vel_a: LuaTable,
    pub vel_b: LuaTable,
    pub rect_a: LuaTable,
    pub rect_b: LuaTable,
    pub signals_a: LuaTable,
    pub signals_b: LuaTable,
    pub signals_a_inner: SignalsCtxTables,
    pub signals_b_inner: SignalsCtxTables,
    pub sides_a: LuaTable,
    pub sides_b: LuaTable,
}

/// Pooled input callback tables, owned directly by `LuaRuntime` and reused across
/// scene, phase, and timer callbacks via cheap `Clone`.
#[derive(Clone)]
pub struct InputCtxTables {
    pub input: LuaTable,
    pub digital: LuaTable,
    pub analog: LuaTable,
    pub up: LuaTable,
    pub down: LuaTable,
    pub left: LuaTable,
    pub right: LuaTable,
    pub action_1: LuaTable,
    pub action_2: LuaTable,
    pub action_3: LuaTable,
    pub back: LuaTable,
    pub special: LuaTable,
    pub main_up: LuaTable,
    pub main_down: LuaTable,
    pub main_left: LuaTable,
    pub main_right: LuaTable,
    pub secondary_up: LuaTable,
    pub secondary_down: LuaTable,
    pub secondary_left: LuaTable,
    pub secondary_right: LuaTable,
    pub debug: LuaTable,
    pub fullscreen: LuaTable,
    pub mouse_left: LuaTable,
}

/// Pooled entity context tables, owned directly by `LuaRuntime` and reused for
/// phase/timer callbacks via cheap `Clone`.
#[derive(Clone)]
pub struct EntityCtxTables {
    pub ctx: LuaTable,
    pub pos: LuaTable,
    pub screen_pos: LuaTable,
    pub vel: LuaTable,
    pub scale: LuaTable,
    pub rect: LuaTable,
    pub sprite: LuaTable,
    pub animation: LuaTable,
    pub timer: LuaTable,
    pub signals: LuaTable,
    pub signals_inner: SignalsCtxTables,
    pub world_pos: LuaTable,
    pub world_scale: LuaTable,
}

/// Resource holding the Lua interpreter state.
///
/// This is a `NonSend` resource because the Lua state is not thread-safe.
/// It should be initialized once at startup and reused throughout the game.
pub struct LuaRuntime {
    pub(super) lua: Lua,
    /// Pooled collision context tables for reuse across collisions.
    collision_ctx_tables: CollisionCtxTables,
    /// Pooled entity context tables for reuse across phase/timer callbacks.
    entity_ctx_tables: EntityCtxTables,
    /// Pooled input callback table reused across Lua callback sites.
    input_ctx_tables: InputCtxTables,
}

/// Converts an [`InputAction`] to its canonical Lua-facing string name.
///
/// These strings are what Lua passes to `engine.rebind_action()` and
/// `engine.get_binding()`.
pub(super) fn action_to_str(action: crate::events::input::InputAction) -> &'static str {
    use crate::events::input::InputAction;
    match action {
        InputAction::MainDirectionUp => "main_up",
        InputAction::MainDirectionDown => "main_down",
        InputAction::MainDirectionLeft => "main_left",
        InputAction::MainDirectionRight => "main_right",
        InputAction::SecondaryDirectionUp => "secondary_up",
        InputAction::SecondaryDirectionDown => "secondary_down",
        InputAction::SecondaryDirectionLeft => "secondary_left",
        InputAction::SecondaryDirectionRight => "secondary_right",
        InputAction::Back => "back",
        InputAction::Action1 => "action_1",
        InputAction::Action2 => "action_2",
        InputAction::Action3 => "action_3",
        InputAction::Special => "special",
        InputAction::ToggleDebug => "toggle_debug",
        InputAction::ToggleFullscreen => "toggle_fullscreen",
    }
}

/// Converts a canonical Lua action name string to an [`InputAction`].
pub fn action_from_str(s: &str) -> Option<crate::events::input::InputAction> {
    use crate::events::input::InputAction;
    match s {
        "up" | "main_up" => Some(InputAction::MainDirectionUp),
        "down" | "main_down" => Some(InputAction::MainDirectionDown),
        "left" | "main_left" => Some(InputAction::MainDirectionLeft),
        "right" | "main_right" => Some(InputAction::MainDirectionRight),
        "secondary_up" => Some(InputAction::SecondaryDirectionUp),
        "secondary_down" => Some(InputAction::SecondaryDirectionDown),
        "secondary_left" => Some(InputAction::SecondaryDirectionLeft),
        "secondary_right" => Some(InputAction::SecondaryDirectionRight),
        "back" => Some(InputAction::Back),
        "action_1" => Some(InputAction::Action1),
        "action_2" => Some(InputAction::Action2),
        "action_3" => Some(InputAction::Action3),
        "special" => Some(InputAction::Special),
        "toggle_debug" => Some(InputAction::ToggleDebug),
        "toggle_fullscreen" => Some(InputAction::ToggleFullscreen),
        _ => None,
    }
}

/// Invokes `$cb!(field)` for each of the 19 digital button fields shared by
/// `DigitalInputs` and `InputCtxTables` (field and table names match for all
/// of them). Used by [`LuaRuntime::diff_digital_tables`] and
/// [`LuaRuntime::write_all_digital_tables`] so the button list is declared
/// exactly once.
macro_rules! for_each_digital_button {
    ($cb:ident) => {
        $cb!(up);
        $cb!(down);
        $cb!(left);
        $cb!(right);
        $cb!(action_1);
        $cb!(action_2);
        $cb!(action_3);
        $cb!(back);
        $cb!(special);
        $cb!(main_up);
        $cb!(main_down);
        $cb!(main_left);
        $cb!(main_right);
        $cb!(secondary_up);
        $cb!(secondary_down);
        $cb!(secondary_left);
        $cb!(secondary_right);
        $cb!(debug);
        $cb!(fullscreen);
        $cb!(mouse_left);
    };
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

        // Set up shared app data for Lua closures to access.
        // Queue fields default to empty vecs; snapshot fields are listed explicitly
        // so their meaningful non-zero defaults (zoom=1.0, fps=60, …) remain visible.
        lua.set_app_data(LuaAppData {
            signal_snapshot: RefCell::new(Arc::new(SignalSnapshot::default())),
            tracked_groups: RefCell::new(FxHashSet::default()),
            gameconfig_snapshot: RefCell::new(GameConfigSnapshot::default()),
            bindings_snapshot: RefCell::new(std::collections::HashMap::new()),
            camera_snapshot: RefCell::new(CameraSnapshot::default()),
            ..Default::default()
        });

        // Create collision context tables for reuse
        let collision_ctx_tables = Self::create_collision_ctx_tables(&lua)?;

        // Create entity context tables for reuse (LuaPhase/LuaTimer)
        let entity_ctx_tables = Self::create_entity_ctx_tables(&lua)?;

        // Create input callback tables for scene/phase/timer callbacks
        let input_ctx_tables = Self::create_input_ctx_tables(&lua)?;

        let runtime = Self {
            lua,
            collision_ctx_tables,
            entity_ctx_tables,
            input_ctx_tables,
        };
        runtime.register_base_api()?;
        runtime.register_asset_api()?;
        runtime.register_spawn_api()?;
        runtime.register_audio_api()?;
        runtime.register_signal_api()?;
        runtime.register_phase_api()?;
        runtime.register_entity_api()?;
        runtime.register_group_api()?;
        runtime.register_camera_api()?;
        runtime.register_camera_follow_api()?;
        runtime.register_collision_api()?;
        runtime.register_animation_api()?;
        runtime.register_render_api()?;
        runtime.register_gameconfig_api()?;
        runtime.register_input_api()?;
        runtime.register_map_api()?;
        runtime.register_builder_meta()?;
        runtime.register_types_meta()?;
        runtime.register_enums_meta()?;
        runtime.register_callbacks_meta()?;

        Ok(runtime)
    }

    /// Creates the pooled collision context tables, reused for every collision.
    fn create_collision_ctx_tables(lua: &Lua) -> LuaResult<CollisionCtxTables> {
        // Create all tables
        let ctx = lua.create_table()?;
        let entity_a = lua.create_table()?;
        let entity_b = lua.create_table()?;
        let pos_a = lua.create_table()?;
        let pos_b = lua.create_table()?;
        let vel_a = lua.create_table()?;
        let vel_b = lua.create_table()?;
        let rect_a = lua.create_table()?;
        let rect_b = lua.create_table()?;
        let signals_a = lua.create_table()?;
        let signals_b = lua.create_table()?;
        let sides = lua.create_table()?;
        let sides_a = lua.create_table()?;
        let sides_b = lua.create_table()?;

        // Wire up entity A structure
        entity_a.set("pos", pos_a.clone())?;
        entity_a.set("vel", vel_a.clone())?;
        entity_a.set("rect", rect_a.clone())?;
        entity_a.set("signals", signals_a.clone())?;

        // Wire up entity B structure
        entity_b.set("pos", pos_b.clone())?;
        entity_b.set("vel", vel_b.clone())?;
        entity_b.set("rect", rect_b.clone())?;
        entity_b.set("signals", signals_b.clone())?;

        // Wire up sides
        sides.set("a", sides_a.clone())?;
        sides.set("b", sides_b.clone())?;

        // Wire up main context
        ctx.set("a", entity_a.clone())?;
        ctx.set("b", entity_b.clone())?;
        ctx.set("sides", sides.clone())?;

        let signals_a_inner = SignalsCtxTables::create(lua)?;
        let signals_b_inner = SignalsCtxTables::create(lua)?;

        Ok(CollisionCtxTables {
            ctx,
            entity_a,
            entity_b,
            pos_a,
            pos_b,
            vel_a,
            vel_b,
            rect_a,
            rect_b,
            signals_a,
            signals_b,
            signals_a_inner,
            signals_b_inner,
            sides_a,
            sides_b,
        })
    }

    /// Returns the pooled collision context tables for reuse.
    /// The caller must populate fields before passing to Lua callbacks.
    pub fn get_collision_ctx_pool(&self) -> CollisionCtxTables {
        self.collision_ctx_tables.clone()
    }

    /// Creates the pooled entity context tables for LuaPhase/LuaTimer callbacks.
    fn create_entity_ctx_tables(lua: &Lua) -> LuaResult<EntityCtxTables> {
        // Create all tables (not wired together since fields are optional)
        Ok(EntityCtxTables {
            ctx: lua.create_table()?,
            pos: lua.create_table()?,
            screen_pos: lua.create_table()?,
            vel: lua.create_table()?,
            scale: lua.create_table()?,
            rect: lua.create_table()?,
            sprite: lua.create_table()?,
            animation: lua.create_table()?,
            timer: lua.create_table()?,
            signals: lua.create_table()?,
            signals_inner: SignalsCtxTables::create(lua)?,
            world_pos: lua.create_table()?,
            world_scale: lua.create_table()?,
        })
    }

    /// Returns the pooled entity context tables for reuse.
    /// The caller must populate fields before passing to Lua callbacks.
    pub fn get_entity_ctx_pool(&self) -> EntityCtxTables {
        self.entity_ctx_tables.clone()
    }

    /// Creates the pooled input callback tables.
    fn create_input_ctx_tables(lua: &Lua) -> LuaResult<InputCtxTables> {
        let input = lua.create_table()?;
        let digital = lua.create_table()?;
        let analog = lua.create_table()?;

        let up = lua.create_table()?;
        let down = lua.create_table()?;
        let left = lua.create_table()?;
        let right = lua.create_table()?;
        let action_1 = lua.create_table()?;
        let action_2 = lua.create_table()?;
        let action_3 = lua.create_table()?;
        let back = lua.create_table()?;
        let special = lua.create_table()?;
        let main_up = lua.create_table()?;
        let main_down = lua.create_table()?;
        let main_left = lua.create_table()?;
        let main_right = lua.create_table()?;
        let secondary_up = lua.create_table()?;
        let secondary_down = lua.create_table()?;
        let secondary_left = lua.create_table()?;
        let secondary_right = lua.create_table()?;
        let debug = lua.create_table()?;
        let fullscreen = lua.create_table()?;
        let mouse_left = lua.create_table()?;

        digital.set("up", up.clone())?;
        digital.set("down", down.clone())?;
        digital.set("left", left.clone())?;
        digital.set("right", right.clone())?;
        digital.set("action_1", action_1.clone())?;
        digital.set("action_2", action_2.clone())?;
        digital.set("action_3", action_3.clone())?;
        digital.set("back", back.clone())?;
        digital.set("special", special.clone())?;
        digital.set("main_up", main_up.clone())?;
        digital.set("main_down", main_down.clone())?;
        digital.set("main_left", main_left.clone())?;
        digital.set("main_right", main_right.clone())?;
        digital.set("secondary_up", secondary_up.clone())?;
        digital.set("secondary_down", secondary_down.clone())?;
        digital.set("secondary_left", secondary_left.clone())?;
        digital.set("secondary_right", secondary_right.clone())?;
        digital.set("debug", debug.clone())?;
        digital.set("fullscreen", fullscreen.clone())?;
        digital.set("mouse_left", mouse_left.clone())?;

        input.set("digital", digital.clone())?;
        input.set("analog", analog.clone())?;

        Ok(InputCtxTables {
            input,
            digital,
            analog,
            up,
            down,
            left,
            right,
            action_1,
            action_2,
            action_3,
            back,
            special,
            main_up,
            main_down,
            main_left,
            main_right,
            secondary_up,
            secondary_down,
            secondary_left,
            secondary_right,
            debug,
            fullscreen,
            mouse_left,
        })
    }

    /// Returns the pooled input callback tables for reuse.
    pub fn get_input_ctx_pool(&self) -> InputCtxTables {
        self.input_ctx_tables.clone()
    }

    fn update_button_table(
        table: &LuaTable,
        state: &super::input_snapshot::DigitalButtonState,
    ) -> LuaResult<()> {
        table.set("pressed", state.pressed)?;
        table.set("just_pressed", state.just_pressed)?;
        table.set("just_released", state.just_released)?;
        Ok(())
    }

    /// Diffs `new` against `old`, calling `update_button_table` for each of
    /// the 18 digital buttons whose state changed.
    fn diff_digital_tables(
        tables: &InputCtxTables,
        old: &super::input_snapshot::DigitalInputs,
        new: &super::input_snapshot::DigitalInputs,
    ) -> LuaResult<()> {
        macro_rules! diff_one {
            ($field:ident) => {
                if old.$field != new.$field {
                    Self::update_button_table(&tables.$field, &new.$field)?;
                }
            };
        }
        for_each_digital_button!(diff_one);
        Ok(())
    }

    /// Writes every digital button table from `snapshot` unconditionally.
    fn write_all_digital_tables(
        tables: &InputCtxTables,
        snapshot: &super::input_snapshot::DigitalInputs,
    ) -> LuaResult<()> {
        macro_rules! write_one {
            ($field:ident) => {
                Self::update_button_table(&tables.$field, &snapshot.$field)?;
            };
        }
        for_each_digital_button!(write_one);
        Ok(())
    }

    fn write_analog_table(
        tables: &InputCtxTables,
        analog: &super::input_snapshot::AnalogInputs,
    ) -> LuaResult<()> {
        tables.analog.set("scroll_y", analog.scroll_y)?;
        tables.analog.set("mouse_x", analog.mouse_x)?;
        tables.analog.set("mouse_y", analog.mouse_y)?;
        tables.analog.set("mouse_world_x", analog.mouse_world_x)?;
        tables.analog.set("mouse_world_y", analog.mouse_world_y)?;
        Ok(())
    }

    /// Updates the pooled input callback table in-place and returns it.
    ///
    /// The returned table is ephemeral, reused across callbacks, and has the
    /// following shape:
    ///
    /// ```lua
    /// input = {
    ///     digital = {
    ///         up = { pressed = bool, just_pressed = bool, just_released = bool },
    ///         -- down, left, right, action_1/2/3, back, special,
    ///         -- main_up/down/left/right, secondary_up/down/left/right,
    ///         -- debug, fullscreen
    ///     },
    ///     analog = {
    ///         scroll_y      = number,  -- mouse wheel delta (positive=up)
    ///         mouse_x       = number,  -- game-space cursor X (0..render_width)
    ///         mouse_y       = number,  -- game-space cursor Y (0..render_height)
    ///         mouse_world_x = number,  -- world-space cursor X (after camera)
    ///         mouse_world_y = number,  -- world-space cursor Y (after camera)
    ///     },
    /// }
    /// ```
    ///
    /// Lua code should treat it as read-only callback data and not retain it
    /// across frames.
    ///
    /// `frame_count` lets repeated calls within the same frame (from
    /// different callback sites) short-circuit entirely, and lets calls on a
    /// new frame diff against the previous frame's snapshot, writing only the
    /// digital buttons and analog values that actually changed.
    pub fn update_input_table(&self, snapshot: &InputSnapshot, frame_count: u64) -> LuaResult<LuaTable> {
        let tables = self.get_input_ctx_pool();

        let data = self
            .lua
            .app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
        let mut last_input = data.last_input.borrow_mut();
        match last_input.as_ref() {
            Some((f, _)) if *f == frame_count => {
                // Already updated for this frame by an earlier call site.
                return Ok(tables.input);
            }
            Some((_, old)) => {
                Self::diff_digital_tables(&tables, &old.digital, &snapshot.digital)?;
                if old.analog != snapshot.analog {
                    Self::write_analog_table(&tables, &snapshot.analog)?;
                }
            }
            None => {
                Self::write_all_digital_tables(&tables, &snapshot.digital)?;
                Self::write_analog_table(&tables, &snapshot.analog)?;
            }
        }
        *last_input = Some((frame_count, snapshot.clone()));

        Ok(tables.input)
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
        match self.get_function(name)? {
            Some(func) => func.call(args),
            None => Err(LuaError::runtime(format!(
                "global function '{name}' not found"
            ))),
        }
    }

    /// Returns a global Lua function if present.
    pub fn get_function(&self, name: &str) -> LuaResult<Option<LuaFunction>> {
        match self.lua.globals().get::<LuaValue>(name)? {
            LuaValue::Nil => Ok(None),
            LuaValue::Function(func) => Ok(Some(func)),
            _ => Err(LuaError::runtime(format!(
                "global '{name}' exists but is not a function"
            ))),
        }
    }

    /// Returns a global Lua function, caching the resolved handle by name.
    ///
    /// Subsequent calls with the same `name` skip the `globals()` lookup
    /// entirely. The cache is invalidated on scene switch via
    /// `clear_function_cache` — callbacks are re-injected per scene, so a
    /// stale handle would otherwise keep pointing at the old scene's closure
    /// environment.
    ///
    /// Caveat: if a script rebinds a global mid-scene, the cached handle wins
    /// until the next scene switch.
    pub fn get_function_cached(&self, name: &str) -> LuaResult<Option<LuaFunction>> {
        let data = self
            .lua
            .app_data_ref::<LuaAppData>()
            .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
        if let Some(f) = data.function_cache.borrow().get(name) {
            return Ok(Some(f.clone()));
        }
        drop(data);

        match self.get_function(name)? {
            Some(f) => {
                let data = self
                    .lua
                    .app_data_ref::<LuaAppData>()
                    .ok_or_else(|| LuaError::runtime("LuaAppData not found"))?;
                data.function_cache
                    .borrow_mut()
                    .insert(name.to_string(), f.clone());
                Ok(Some(f))
            }
            None => Ok(None),
        }
    }

    /// Resolves `name` via [`get_function_cached`](Self::get_function_cached) and invokes it
    /// through `f`, centralizing the not-found/error logging shared by every callback dispatch
    /// site (phase, timer, setup, animation-end).
    ///
    /// `label` identifies the callback kind for the "not found" warning (e.g. `"Phase"`,
    /// `"Timer"`). Returns `None` if the callback is missing or resolving/calling it errors;
    /// the error is logged in both cases.
    pub fn call_named<R, F>(&self, name: &str, label: &str, f: F) -> Option<R>
    where
        F: FnOnce(LuaFunction) -> LuaResult<R>,
    {
        match self.get_function_cached(name) {
            Ok(Some(func)) => match f(func) {
                Ok(r) => Some(r),
                Err(e) => {
                    log::error!(target: "lua", "Error in {}(): {}", name, e);
                    None
                }
            },
            Ok(None) => {
                log::warn!(target: "lua", "{} callback '{}' not found", label, name);
                None
            }
            Err(e) => {
                log::error!(target: "lua", "Error resolving {}(): {}", name, e);
                None
            }
        }
    }

    /// Clears cached function handles (see `get_function_cached`). Call on
    /// scene switch, alongside `clear_all_commands`.
    pub fn clear_function_cache(&self) {
        if let Some(data) = self.lua.app_data_ref::<LuaAppData>() {
            data.function_cache.borrow_mut().clear();
        }
    }

    /// Checks if a global function exists.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the function to check
    pub fn has_function(&self, name: &str) -> bool {
        self.get_function(name)
            .map(|func| func.is_some())
            .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_theme_commands_preserve_policy_survives_clear_render_commands_does_not() {
        let runtime = LuaRuntime::new().unwrap();
        if let Some(data) = runtime.lua().app_data_ref::<LuaAppData>() {
            data.gui_theme_commands.borrow_mut().push(RenderCmd::SetGuiThemeFont {
                theme_key: "default".to_string(),
                font_key: "arcade".to_string(),
                font_size: 16.0,
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            });
            data.render_commands
                .borrow_mut()
                .push(RenderCmd::ClearPostProcessUniforms);
        }

        // Mirrors switch_scene's clear_all_commands() call, which runs
        // before commands queued during on_setup() (or earlier this frame)
        // are ever drained.
        runtime.clear_all_commands();

        let mut render_buf = Vec::new();
        runtime.drain_render_commands_into(&mut render_buf);
        assert!(
            render_buf.is_empty(),
            "render_commands (clear policy) must be wiped by clear_all_commands"
        );

        let mut theme_buf = Vec::new();
        runtime.drain_gui_theme_commands_into(&mut theme_buf);
        assert_eq!(
            theme_buf.len(),
            1,
            "gui_theme_commands (preserve policy) must survive clear_all_commands, \
             so engine.set_gui_theme_* calls queued from on_setup() aren't lost"
        );
    }

    #[test]
    fn pooled_input_table_updates_values() {
        let runtime = LuaRuntime::new().unwrap();
        let mut snapshot = InputSnapshot::default();
        snapshot.digital.action_1.pressed = true;
        snapshot.digital.action_1.just_pressed = true;
        snapshot.analog.mouse_x = 12.5;
        snapshot.analog.mouse_world_y = -4.0;

        let input = runtime.update_input_table(&snapshot, 1).unwrap();
        let digital: LuaTable = input.get("digital").unwrap();
        let action_1: LuaTable = digital.get("action_1").unwrap();
        let analog: LuaTable = input.get("analog").unwrap();

        assert!(action_1.get::<bool>("pressed").unwrap());
        assert!(action_1.get::<bool>("just_pressed").unwrap());
        assert_eq!(analog.get::<f32>("mouse_x").unwrap(), 12.5);
        assert_eq!(analog.get::<f32>("mouse_world_y").unwrap(), -4.0);
    }

    #[test]
    fn pooled_input_table_reuses_same_lua_table() {
        let runtime = LuaRuntime::new().unwrap();
        let first = runtime
            .update_input_table(&InputSnapshot::default(), 1)
            .unwrap();

        let mut snapshot = InputSnapshot::default();
        snapshot.digital.back.just_pressed = true;
        let second = runtime.update_input_table(&snapshot, 2).unwrap();

        let globals = runtime.lua().globals();
        globals.set("first_input", first).unwrap();
        globals.set("second_input", second).unwrap();

        let same_identity = runtime
            .lua()
            .load("return first_input == second_input")
            .eval::<bool>()
            .unwrap();
        assert!(same_identity);
    }

    #[test]
    fn update_input_table_is_noop_within_same_frame() {
        let runtime = LuaRuntime::new().unwrap();

        let mut snapshot = InputSnapshot::default();
        snapshot.digital.action_1.pressed = true;
        let input = runtime.update_input_table(&snapshot, 7).unwrap();
        let digital: LuaTable = input.get("digital").unwrap();
        let action_1: LuaTable = digital.get("action_1").unwrap();
        assert!(action_1.get::<bool>("pressed").unwrap());

        // Mutate the table directly, then call again with the same frame
        // count — the second call must be a no-op and not overwrite our
        // out-of-band change.
        action_1.set("pressed", false).unwrap();
        let input_again = runtime.update_input_table(&snapshot, 7).unwrap();
        let digital_again: LuaTable = input_again.get("digital").unwrap();
        let action_1_again: LuaTable = digital_again.get("action_1").unwrap();
        assert!(!action_1_again.get::<bool>("pressed").unwrap());
    }

    #[test]
    fn update_input_table_diffs_on_new_frame() {
        let runtime = LuaRuntime::new().unwrap();

        let snapshot_a = InputSnapshot::default();
        let input = runtime.update_input_table(&snapshot_a, 1).unwrap();
        let digital: LuaTable = input.get("digital").unwrap();
        let action_1: LuaTable = digital.get("action_1").unwrap();
        let back: LuaTable = digital.get("back").unwrap();
        assert!(!action_1.get::<bool>("pressed").unwrap());
        assert!(!back.get::<bool>("pressed").unwrap());

        let mut snapshot_b = InputSnapshot::default();
        snapshot_b.digital.action_1.pressed = true;
        snapshot_b.digital.action_1.just_pressed = true;
        let _ = runtime.update_input_table(&snapshot_b, 2).unwrap();

        // Changed button reflects the new state.
        assert!(action_1.get::<bool>("pressed").unwrap());
        assert!(action_1.get::<bool>("just_pressed").unwrap());
        // Unchanged button is untouched (still false).
        assert!(!back.get::<bool>("pressed").unwrap());
    }

    #[test]
    fn get_function_cached_returns_callable_function() {
        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load("function greet() return 'hello' end")
            .exec()
            .unwrap();

        let first = runtime.get_function_cached("greet").unwrap().unwrap();
        let second = runtime.get_function_cached("greet").unwrap().unwrap();

        assert_eq!(first.call::<String>(()).unwrap(), "hello");
        assert_eq!(second.call::<String>(()).unwrap(), "hello");
    }

    #[test]
    fn clear_function_cache_picks_up_redefined_global() {
        let runtime = LuaRuntime::new().unwrap();
        runtime
            .lua()
            .load("function greet() return 'old' end")
            .exec()
            .unwrap();

        let cached = runtime.get_function_cached("greet").unwrap().unwrap();
        assert_eq!(cached.call::<String>(()).unwrap(), "old");

        runtime
            .lua()
            .load("function greet() return 'new' end")
            .exec()
            .unwrap();

        // Cache still holds the old handle until cleared.
        let still_old = runtime.get_function_cached("greet").unwrap().unwrap();
        assert_eq!(still_old.call::<String>(()).unwrap(), "old");

        runtime.clear_function_cache();

        let refreshed = runtime.get_function_cached("greet").unwrap().unwrap();
        assert_eq!(refreshed.call::<String>(()).unwrap(), "new");
    }
}
