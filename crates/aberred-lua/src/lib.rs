//! Lua scripting layer for Aberred Engine.
//!
//! This crate hosts the Lua runtime ([`resources::lua_runtime`]), the
//! command-queue drain/dispatch machinery ([`systems::lua_commands`]),
//! per-callback observers/systems ([`systems`]), the Lua-only components
//! ([`components`])/events ([`events`]) those observers react to, the
//! bootstrap glue ([`lua_plugin`]), and the LSP-stub/`.luarc.json` codegen
//! tools ([`stub_generator`], [`luarc_generator`]).
//!
//! Depends only on `aberred-core` -- no `aberred-render`/`aberred-audio`
//! type ever crosses into Lua-visible state; render/audio-bound data flows
//! exclusively through `aberred_core::protocol`'s wire types. This is also
//! the only crate in the workspace allowed to use macro-based codegen
//! (`resources::lua_runtime::queue_registry`'s `lua_queues!`,
//! `resources::lua_runtime::entity_builder`'s `builder_method!`) -- see
//! `docs/plans/workspaces-implementation.md`'s Phase 5 decision record.
//!
//! The facade crate (`aberredengine`) keeps four small Lua-priority "shadow"
//! systems of its own (`systems::{menu, gui_interactable_click,
//! collision_rule_index, mapspawn}`) that call into this crate's
//! `systems::lua_*` modules -- those shadow files never moved here, since
//! they also need to resolve to a Rust-only variant when the `lua` feature
//! is off, which this crate (Lua-only by construction) cannot express.

pub mod components;
pub mod events;
pub mod lua_plugin;
pub mod luarc_generator;
pub mod resources;
pub mod stub_generator;
pub mod systems;
