//! Engine bootstrapping via the builder pattern.
//!
//! [`EngineBuilder`] captures all the boilerplate in `main.rs` — world setup,
//! window init, resources, system schedule, and main loop — into a single
//! configurable struct. The developer supplies only game-specific hooks.
//!
//! # Examples
//!
//! **Lua game:**
//! ```rust,no_run
//! # #[cfg(feature = "lua")]
//! # fn main() {
//! use aberredengine::engine_app::EngineBuilder;
//!
//! EngineBuilder::new()
//!     .with_lua("assets/scripts/main.lua")
//!     .run();
//! # }
//! # #[cfg(not(feature = "lua"))]
//! # fn main() {}
//! ```
//!
//! **Pure Rust game** (one implicit scene, `"main"`):
//! ```rust,ignore
//! use aberredengine::prelude::*;
//!
//! fn main() -> Result<(), EngineError> {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .title("My Game")
//!         .on_setup(my_game::setup)
//!         .on_scene_enter("main", my_game::spawn_world)
//!         .add_system(my_game::update)
//!         .try_run()
//! }
//! ```
//!
//! **Scenes, per-tick systems and custom observers:**
//! ```rust,ignore
//! use aberredengine::prelude::*;
//!
//! fn main() -> Result<(), EngineError> {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .on_setup(load_assets)
//!         .add_system(tilemap_load_system)   // runs once per sim tick while Playing
//!         .add_system(tilemap_save_system)   // multiple systems allowed
//!         .add_observer(on_tilemap_loaded)   // persistent observer for a custom event
//!         .add_scene("intro")
//!         .add_scene("editor")
//!         .on_scene_enter("editor", spawn_editor)  // SceneEntered, "editor" only
//!         .add_scene_system("editor", editor_tools) // per tick while "editor" is active
//!         .initial_scene("intro")
//!         .try_run()
//! }
//! ```
//!
//! For scene-scoped (transient) observers — active only within one scene —
//! spawn them from a `SceneEntered` observer without [`Persistent`](aberred_core::components::persistent::Persistent):
//! ```rust,ignore
//! fn spawn_editor(_: On<SceneEntered>, mut commands: Commands) {
//!     // Despawned with the scene on the next scene switch
//!     commands.spawn(Observer::new(on_my_scene_event));
//! }
//! ```
//!
//! # Module layout
//!
//! This module mirrors the shape of `aberred_lua::resources::lua_runtime`: one
//! directory module with a single struct ([`EngineBuilder`]) whose `impl`
//! block is split across sibling files by concern, plus a runtime-support
//! file (`logic_thread`) and the test suite.
//!
//! - `builder` — `struct EngineBuilder` and its fluent builder-pattern API.
//! - `registrar` — the boxed-closure types (`HookRegistrar`/
//!   `UpdateRegistrar`/`ObserverRegistrar`) and `hook_registrar`, used to
//!   defer system registration until the `World` exists.
//! - `run` — `run`/`try_run` (the builder's terminal methods) and the
//!   render thread's per-frame main loop.
//! - `validate` — preflight validation of builder state before startup.
//! - `config` — config-file loading and raylib window/log-level setup.
//! - [`aberred_render::bootstrap`] (not a module here) — bootstrapping the
//!   render (main) thread's `World` and its per-frame schedule
//!   (`setup_render_world` + `build_render_schedule`, which `try_run` always
//!   calls back to back).
//! - `logic_world` — bootstrapping the logic thread's gameplay `World`,
//!   registering hooks/scene systems, and spawning engine observers.
//! - `schedule` — [`SimSet`] and construction of the logic thread's `sim`/
//!   `present` schedules.
//! - `logic_thread` — `LogicInit`, the logic thread's entry point, and its
//!   `Pacer`-driven main loop.
//!
//! Unlike `aberred_lua::resources::lua_runtime`'s single flat re-export tier,
//! this module's public surface is intentionally two-tiered: a real `pub`
//! external API ([`EngineBuilder`], [`SimSet`]) plus a `pub(crate)`,
//! test-support-gated internal tier (`LogicInit`, `run_sim_tick`, the
//! registrar types) that only `src/test_support.rs` consumes — `lua_runtime`
//! has no equivalent test-only internals to gate.

mod builder;
mod config;
mod logic_thread;
mod logic_world;
mod registrar;
mod replay;
mod run;
mod schedule;
#[cfg(test)]
mod tests;
mod validate;

// External API surface — keeps `crate::engine_app::EngineBuilder` unchanged
// for src/main.rs, doc examples, and downstream games.
pub use builder::EngineBuilder;
pub use schedule::SimSet;

// Crate-internal surface — exactly what src/test_support.rs imports today.
// Only consumed there, which is itself `#[cfg(any(test, feature =
// "test-support"))]` — gate identically so a default `cargo build` doesn't
// warn these as unused (the original flat file had no equivalent warning,
// since these were inline item definitions rather than `use` re-exports).
#[cfg(any(test, feature = "test-support"))]
pub(crate) use logic_thread::{
    LogicInit, apply_tick_input, drain_logic_messages, hold_back_deterministic_setup_input,
    in_envelope, run_sim_tick,
};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use registrar::{
    HookRegistrar, ObserverRegistrar, UpdateRegistrar, hook_registrar, observer_registrar,
    scene_observer_registrar, system_registrar,
};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use replay::{ReplayPlayer, ReplayRecorder, validate_replay_header};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use validate::ensure_main_scene;
