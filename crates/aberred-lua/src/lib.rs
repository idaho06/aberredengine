//! Lua scripting layer for Aberred Engine.
//!
//! This crate hosts the Lua runtime ([`resources::lua_runtime`]), the
//! command-queue drain/dispatch machinery ([`systems::lua_commands`]),
//! per-callback observers/systems ([`systems`]), the Lua-only callback
//! components ([`components`]) those observers read, the
//! bootstrap glue ([`lua_plugin`]), and the LSP-stub/`.luarc.json` codegen
//! tools ([`stub_generator`], [`luarc_generator`]).
//!
//! Depends only on `aberred-core` -- no `aberred-render`/`aberred-audio`
//! type ever crosses into Lua-visible state; render/audio-bound data flows
//! exclusively through `aberred_core::protocol`'s wire types. This is also
//! the only crate in the workspace allowed to use macro-based codegen
//! (`resources::lua_runtime::queue_registry`'s `lua_queues!`,
//! `resources::lua_runtime::entity_builder`'s `builder_method!`). Project
//! direction confines macro-based codegen to this crate; core, render, and
//! audio use generic functions instead.
//!
//! Every Lua system and observer here runs on top of core's, never instead
//! of it: the facade registers them only when the game runs a Lua script.

pub mod components;
pub mod lua_plugin;
pub mod luarc_generator;
pub mod resources;
pub mod stub_generator;
pub mod systems;
