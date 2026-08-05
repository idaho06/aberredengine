//! Bridge systems that shuttle `AudioCmd`/`AudioMessage` between the LOGIC
//! (sim) thread and the dedicated audio thread ([`crate::systems::audio`]).
//!
//! These four systems run on the logic thread, not the audio thread -- they
//! never touch `AudioStore`/`PlayingFx`/`MusicTrack`, which are internal to
//! `crate::systems::audio`'s own `World` and never cross into the sim world.
//!
//! - [`poll_audio_messages`] non-blockingly drains the audio thread's event
//!   receiver into Bevy ECS' message queue each frame.
//! - [`update_bevy_audio_messages`] advances the ECS message queue so newly
//!   written messages become readable by message subscribers.
//! - [`forward_audio_cmds`] forwards ECS `AudioCmd` messages to the audio
//!   thread via the `AudioBridge` sender.
//! - [`update_bevy_audio_cmds`] advances the ECS message queue for `AudioCmd`
//!   so same-frame readers can observe writes.
//! - [`land_audio_stats`] reads `AudioMessage::Stats` out of the queue into
//!   the logic-world `AudioStats` resource, for the F11 perf panel.

use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::protocol::endpoints::AudioBridge;
use crate::resources::thread_stats::AudioStats;
use bevy_ecs::prelude::Messages;
use bevy_ecs::{
    prelude::{MessageReader, MessageWriter, Res},
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

/// Mirror the audio thread's latest tick-stats rollup into the logic-world
/// `AudioStats` resource. Run after [`update_bevy_audio_messages`] so this
/// frame's freshly-written `AudioMessage::Stats` (if any) is visible here.
pub fn land_audio_stats(mut reader: MessageReader<AudioMessage>, mut stats: ResMut<AudioStats>) {
    for msg in reader.read() {
        if let AudioMessage::Stats { stats: s } = msg {
            stats.0 = *s;
        }
    }
}
