//! Lua-priority shadow systems.
//!
//! The bulk of the engine's systems live in `aberred_core::systems`
//! (re-exported as `aberredengine::core::systems`). This module holds one
//! "shadow" module (`mapspawn`) that re-exports core's `spawn_map_observer`,
//! then overrides it under `#[cfg(feature = "lua")]` with a Lua-priority
//! variant from the `aberred-lua` crate -- core keeps a Rust-only version
//! unconditionally, since it cannot name Lua-only component types at all.
//! Every other Lua-callback system/command-processing module lives in the
//! `aberred-lua` crate (re-exported as `aberredengine::lua::systems`). The audio thread's own `bevy_ecs::World`
//! and its systems live in the `aberred-audio` crate; render (main) thread
//! systems live in the `aberred-render` crate, which the facade does not
//! re-export (its `render` module holds only callback-facing types).

pub mod mapspawn;
