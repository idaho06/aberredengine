//! ECS resources that bridge the main thread with the background audio thread.
//!
//! Use [`setup_audio`] once during initialization to spawn the audio thread
//! and insert the [`AudioBridge`] and `Messages<AudioMessage>` resources. Call
//! [`shutdown_audio`] during teardown to gracefully stop the thread and free
//! audio resources.

use crate::events::audio::{AudioCmd, AudioMessage};
use crate::systems::audio::audio_thread;
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};

/// Shared bridge between the ECS world and the audio thread.
///
/// This resource is created by [`setup_audio`]. Systems can send commands via
/// [`AudioBridge::tx_cmd`] and poll for events via [`AudioBridge::rx_msg`].
#[derive(Resource)]
pub struct AudioBridge {
    /// Sender for [`AudioCmd`] messages (ECS -> audio thread).
    pub tx_cmd: Sender<AudioCmd>,
    /// Receiver for [`AudioMessage`] messages (audio thread -> ECS).
    pub rx_msg: Receiver<AudioMessage>,
    /// Join handle for the background audio thread.
    pub handle: std::thread::JoinHandle<()>,
}

/// Spawn the audio thread and register bridge resources.
///
/// This function:
/// - Creates command/event channels.
/// - Spawns the background thread running [`audio_thread`].
/// - Inserts [`AudioBridge`] and initializes `Messages<AudioMessage>` so that
///   systems can send commands and poll for events.
pub fn setup_audio(world: &mut World) {
    let (tx_cmd, rx_cmd) = unbounded::<AudioCmd>();
    let (tx_msg, rx_msg) = unbounded::<AudioMessage>();

    let handle = std::thread::spawn(move || audio_thread(rx_cmd, tx_msg));

    world.insert_resource(AudioBridge {
        tx_cmd,
        rx_msg,
        handle,
    });
    world.insert_resource(Messages::<AudioMessage>::default());
    world.insert_resource(Messages::<AudioCmd>::default());
}

/// Gracefully request shutdown of the audio thread and join it.
///
/// If the bridge resource exists, sends [`AudioCmd::Shutdown`], waits for the
/// thread to exit, and removes the resource from the world.
pub fn shutdown_audio(world: &mut World) {
    if let Some(bridge) = world.remove_resource::<AudioBridge>() {
        let _ = bridge.tx_cmd.send(AudioCmd::Shutdown);
        let _ = bridge.handle.join();
    }
}
