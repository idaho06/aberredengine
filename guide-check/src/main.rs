//! Every ```rust fence of RUST-GAME-GUIDE.md, compiled as a downstream game.
//!
//! Each fence sits verbatim in its own module, grouped by guide section. Lines
//! marked `// GLUE` are not in the guide: imports a snippet relies on from an
//! earlier snippet, stand-ins for names the guide leaves to the reader, or a
//! wrapper fn around a statement fragment. Nothing here runs a game.
#![allow(unused, dead_code)]

mod assets;
mod gameplay;
mod gui;
mod resources;
mod scene_manager;
mod scenes;
mod spawning;
mod testing;

fn main() {}
