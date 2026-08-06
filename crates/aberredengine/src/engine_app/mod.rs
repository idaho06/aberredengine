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
//! **Pure Rust game:**
//! ```rust,no_run,ignore
//! use aberredengine::engine_app::EngineBuilder;
//! use aberredengine::EngineError;
//!
//! fn main() -> Result<(), EngineError> {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .title("My Game")
//!         .on_setup(my_game::setup)
//!         .on_enter_play(my_game::enter_play)
//!         .on_update(my_game::update)
//!         .on_switch_scene(my_game::switch_scene)
//!         .try_run()
//! }
//! ```
//!
//! **Multiple per-frame systems and custom observers:**
//! ```rust,no_run,ignore
//! use aberredengine::engine_app::EngineBuilder;
//! use aberredengine::engine_app::SceneDescriptor;
//! use aberredengine::EngineError;
//!
//! fn main() -> Result<(), EngineError> {
//!     EngineBuilder::new()
//!         .config("config.ini")
//!         .on_setup(load_assets)
//!         .add_system(tilemap_load_system)   // runs once per sim tick while Playing
//!         .add_system(tilemap_save_system)   // multiple systems allowed
//!         .add_observer(on_tilemap_loaded)   // persistent observer for a custom event
//!         .add_scene("intro", SceneDescriptor { /* … */ })
//!         .add_scene("editor", SceneDescriptor { /* … */ })
//!         .initial_scene("intro")
//!         .try_run()
//! }
//! ```
//!
//! For scene-scoped (transient) observers — active only within one scene —
//! spawn them from the scene's `on_enter` callback without [`Persistent`]:
//! ```rust,no_run,ignore
//! fn my_scene_enter(ctx: &mut GameCtx) {
//!     // Cleaned up automatically by clean_all_entities on scene switch
//!     ctx.commands.spawn(Observer::new(on_my_scene_event));
//! }
//! ```
//!
//! # Module layout
//!
//! This module mirrors the shape of [`aberred_lua::resources::lua_runtime`]: one
//! directory module with a single struct ([`EngineBuilder`]) whose `impl`
//! block is split across sibling files by concern, plus a runtime-support
//! file ([`logic_thread`]) and the test suite.
//!
//! - [`builder`] — `struct EngineBuilder` and its fluent builder-pattern API.
//! - [`registrar`] — the boxed-closure types (`HookRegistrar`/
//!   `UpdateRegistrar`/`ObserverRegistrar`) and `hook_registrar`, used to
//!   defer system registration until the `World` exists.
//! - [`run`] — `run`/`try_run` (the builder's terminal methods) and the
//!   render thread's per-frame main loop.
//! - [`validate`] — preflight validation of builder state before startup.
//! - [`config`] — config-file loading and raylib window/log-level setup.
//! - [`render_world`] — bootstrapping the render (main) thread's `World` and
//!   its per-frame schedule (`build_render_schedule` lives here, alongside
//!   `setup_render_world`, since `try_run` always calls the pair back to
//!   back — see [`.claude/context/system-order.md`]'s RENDER schedule
//!   section for what each step in that schedule does).
//! - [`logic_world`] — bootstrapping the logic thread's gameplay `World`,
//!   registering hooks/scene systems, and spawning engine observers.
//! - [`schedule`] — [`SimSet`] and construction of the logic thread's `sim`/
//!   `present` schedules (see `.claude/context/system-order.md`'s Sim/PRESENT
//!   schedule sections).
//! - [`logic_thread`] — `LogicInit`, the logic thread's entry point, and its
//!   `Pacer`-driven main loop.
//!
//! Unlike [`aberred_lua::resources::lua_runtime`]'s single flat re-export tier,
//! this module's public surface is intentionally two-tiered: a real `pub`
//! external API ([`EngineBuilder`], [`SimSet`]) plus a `pub(crate)`,
//! test-support-gated internal tier ([`LogicInit`], `run_sim_tick`, the
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
mod scene;
#[cfg(test)]
mod tests;
mod validate;

// External API surface — keeps `crate::engine_app::EngineBuilder` unchanged
// for src/main.rs, doc examples, and downstream games.
pub use builder::EngineBuilder;
pub use scene::SceneDescriptor;
pub use schedule::SimSet;

// Crate-internal surface — exactly what src/test_support.rs imports today.
// Only consumed there, which is itself `#[cfg(any(test, feature =
// "test-support"))]` — gate identically so a default `cargo build` doesn't
// warn these as unused (the original flat file had no equivalent warning,
// since these were inline item definitions rather than `use` re-exports).
#[cfg(any(test, feature = "test-support"))]
pub(crate) use logic_thread::{LogicInit, apply_tick_input, run_sim_tick};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use registrar::{HookRegistrar, ObserverRegistrar, UpdateRegistrar, hook_registrar};
#[cfg(any(test, feature = "test-support"))]
pub(crate) use replay::{ReplayPlayer, ReplayRecorder, validate_replay_header};
