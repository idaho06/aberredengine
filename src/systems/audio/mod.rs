//! Audio system implementation backed by a dedicated thread and Raylib.
//!
//! This module hosts the background audio thread and the systems that bridge
//! it with the ECS world:
//! - [`audio_thread`] runs on its own OS thread as its own `bevy_ecs::World`
//!   (Phase 7e), owns the Raylib audio device, and processes [`AudioCmd`]
//!   messages, emitting [`AudioMessage`] responses.
//! - [`poll_audio_messages`] non-blockingly drains the audio thread's event
//!   receiver into Bevy ECS' message queue each frame.
//! - [`update_bevy_audio_messages`] advances the ECS message queue so newly
//!   written messages become readable by message subscribers.
//!
//! The design keeps Raylib audio API calls isolated to a single thread, while
//! the main game thread communicates via lock-free channels. These four
//! bridge functions run on the LOGIC (sim) thread, not the audio thread --
//! they never touch `AudioStore`/`PlayingFx`/`MusicTrack`, which are internal
//! to [`world`] and never cross into the sim world.
//!
//! Notes
//! - The audio thread must be created once via
//!   [`crate::protocol::endpoints::setup_audio`] and joined/terminated via
//!   [`crate::protocol::endpoints::shutdown_audio`].
//! - All file I/O (load) and control (play/stop/pause/volume) happen on the
//!   audio thread in response to commands.
//! - Music streaming requires periodic `update_stream()` calls; the audio
//!   world's `pump_music` system takes care of it while tracks are playing.
//! - The loop is `Pacer`-driven (Phase 7b): it wakes at a configurable
//!   `[audio] hz` rate (`config.ini`) rather than blocking on the command
//!   channel, draining all pending commands non-blockingly each tick and
//!   then pumping music streams / cleaning up finished sound aliases.
//!   Accepted tradeoff: constant wakeups at `audio_hz` instead of blocking
//!   while idle (negligible at the default 100Hz with `spin_sleep`).
//!
//! See also: [`crate::protocol::audio`] and [`crate::protocol::endpoints`].

mod store;
mod systems;
mod world;

pub use world::audio_thread;

use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::protocol::endpoints::AudioBridge;
use bevy_ecs::prelude::Messages;
use bevy_ecs::{
    prelude::{MessageWriter, Res},
    system::ResMut,
};

/// Drain any pending events from the audio thread and enqueue them into the
/// ECS [`Messages<AudioMessage>`] mailbox.
///
/// This is a non-blocking system function intended to run each frame on the
/// main thread. It ensures that messages produced by the audio thread become
/// available to ECS message readers and systems that consume
/// [`AudioMessage`].
///
/// It does not mutate world state beyond writing messages.
pub fn poll_audio_messages(bridge: Res<AudioBridge>, mut writer: MessageWriter<AudioMessage>) {
    crate::tracy::tracy_span!("poll_audio_messages");
    writer.write_batch(bridge.rx_msg.try_iter());
}

/// Advance the ECS message queue for [`AudioMessage`].
///
/// Bevy ECS' [`Messages`] API requires calling `update()` once per frame to
/// make messages written this frame visible to readers in the same frame.
/// Run this after [`poll_audio_messages`] in your schedule.
pub fn update_bevy_audio_messages(mut msgs: ResMut<Messages<AudioMessage>>) {
    msgs.update();
}

/// Forward ECS AudioCmd messages to the audio thread via the AudioBridge sender.
pub fn forward_audio_cmds(
    bridge: Res<AudioBridge>,
    mut reader: bevy_ecs::prelude::MessageReader<AudioCmd>,
) {
    crate::tracy::tracy_span!("forward_audio_cmds");
    for cmd in reader.read() {
        // Forward clone to crossbeam channel; ignore send error on shutdown
        let _ = bridge.tx_cmd.send(cmd.clone());
    }
}

/// Advance the ECS message queue for AudioCmd so same-frame readers can observe writes.
pub fn update_bevy_audio_cmds(mut msgs: ResMut<Messages<AudioCmd>>) {
    msgs.update();
}
