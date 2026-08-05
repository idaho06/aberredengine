//! [`audio_thread`] runs on its own OS thread, owns the Raylib audio
//! device, and processes [`AudioCmd`] messages, emitting [`AudioMessage`]
//! responses (see the crate root docs for how this fits into the engine).
//!
//! The bridge functions that run on the LOGIC (sim) thread to shuttle
//! [`AudioCmd`]/[`AudioMessage`] across the channel -- and never touch
//! `AudioStore`/`PlayingFx`/`MusicTrack`, which are internal to this module's
//! `World` and never cross into the sim world -- live in
//! `aberred_core::systems::audio_bridge`, not here.
//!
//! Notes
//! - The audio thread must be created once via [`setup_audio`] and
//!   joined/terminated via
//!   [`aberred_core::protocol::endpoints::shutdown_audio`].
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
//! See also: [`aberred_core::protocol::audio`] and [`aberred_core::protocol::endpoints`].

mod pipeline;
mod world;

pub use world::audio_thread;

use aberred_core::protocol::audio::{AudioCmd, AudioMessage};
use aberred_core::protocol::endpoints::insert_audio_bridge_resources;
use bevy_ecs::world::World;
use crossbeam_channel::unbounded;

/// Spawn the audio thread and register bridge resources.
///
/// This function:
/// - Creates command/event channels.
/// - Spawns the background thread running [`audio_thread`], paced at
///   `audio_hz`, read once here at spawn time -- a runtime change to
///   `GameConfig::audio_hz` afterward has no effect.
/// - Inserts `AudioBridge` and initializes `Messages<AudioMessage>` so that
///   systems can send commands and poll for events.
///
/// Lives in `aberred-audio`, not `aberred-core`, because [`audio_thread`]
/// owns a Raylib audio device -- `aberred-core` cannot depend on Raylib.
pub fn setup_audio(world: &mut World, audio_hz: f64) {
    let (tx_cmd, rx_cmd) = unbounded::<AudioCmd>();
    let (tx_msg, rx_msg) = unbounded::<AudioMessage>();

    let handle = std::thread::spawn(move || audio_thread(rx_cmd, tx_msg, audio_hz));

    insert_audio_bridge_resources(world, tx_cmd, rx_msg, handle);
}
