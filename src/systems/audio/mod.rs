//! Audio thread implementation backed by a dedicated thread and Raylib.
//!
//! This module hosts the background audio thread and its own `bevy_ecs::World`:
//! [`audio_thread`] runs on its own OS thread, owns the Raylib
//! audio device, and processes [`AudioCmd`] messages, emitting [`AudioMessage`]
//! responses.
//!
//! The bridge functions that run on the LOGIC (sim) thread to shuttle
//! [`AudioCmd`]/[`AudioMessage`] across the channel -- and never touch
//! `AudioStore`/`PlayingFx`/`MusicTrack`, which are internal to this module's
//! `World` and never cross into the sim world -- live in
//! [`crate::systems::audio_bridge`], not here.
//!
//! Notes
//! - The audio thread must be created once via
//!   [`crate::protocol::endpoints::setup_audio`] and joined/terminated via
//!   [`crate::protocol::endpoints::shutdown_audio`].
//! - All file I/O (load) and control (play/stop/pause/volume) happen on the
//!   audio thread in response to commands.
//! - Music streaming requires periodic `update_stream()` calls; the audio
//!   world's `pump_music` system takes care of it while tracks are playing.
//! - The loop is `Pacer`-driven: it wakes at a configurable
//!   `[audio] hz` rate (`config.ini`) rather than blocking on the command
//!   channel, draining all pending commands non-blockingly each tick and
//!   then pumping music streams / cleaning up finished sound aliases.
//!   Accepted tradeoff: constant wakeups at `audio_hz` instead of blocking
//!   while idle (negligible at the default 100Hz with `spin_sleep`).
//!
//! See also: [`crate::protocol::audio`] and [`crate::protocol::endpoints`].

mod systems;
mod world;

pub use world::audio_thread;
