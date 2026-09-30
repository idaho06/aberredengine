//! Shared constants and helpers for integration tests under `tests/` (each
//! sibling file there is its own compiled crate, so this lives in a
//! `common/mod.rs` subdirectory -- the standard Cargo idiom for sharing code
//! between integration test binaries without `common` itself becoming one).
//!
//! Each consuming binary compiles the whole module, so `dead_code` fires in
//! any binary that declares `mod common;` without using everything in it --
//! allowed here rather than at every call site.
#![allow(dead_code)]

/// Fixed per-tick `dt` used by `TestWorld::tick`/`tick_to_play` calls across
/// the integration test suite (60Hz-equivalent).
pub const DT: f32 = 1.0 / 60.0;
