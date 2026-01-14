//! Event types and observers used by the engine.
//!
//! This module groups the domain events exchanged across systems and the
//! corresponding observers that react to them. Events provide a decoupled
//! way for systems to communicate without tight coupling or direct
//! dependencies.
//!
//! Submodules:
//! - [`audio`] – commands and messages for the background audio thread
//! - [`collision`] – collision notifications emitted by the physics/collision system
//! - [`gamestate`] – state transition notifications for the high-level game flow
//! - [`input`] – input action events (key press/release)
//! - [`menu`] – menu selection events
//! - [`luatimer`] – Lua timer callback events
//! - [`switchdebug`] – toggle debug rendering and diagnostics on/off
//! - [`switchfullscreen`] – toggle fullscreen mode on/off
//!
//! See each submodule for concrete event data, semantics, and example usage.

pub mod audio;
pub mod collision;
pub mod gamestate;
pub mod input;
pub mod luatimer;
pub mod menu;
pub mod switchdebug;
pub mod switchfullscreen;
