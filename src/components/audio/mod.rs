//! Components used only by the audio thread's own `bevy_ecs::World` (Phase
//! 7e). Nothing here is ever inserted into or read from the sim/logic
//! world -- the only contract between the two worlds is
//! `AudioCmd`/`AudioMessage` (`crate::protocol::audio`).

pub mod handles;
pub mod music_track;
pub mod playing_fx;
