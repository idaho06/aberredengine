use crate::events::audio::{AudioCmd, AudioEvent};
use crate::systems::audio::audio_thread;
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender, unbounded};

#[derive(Resource)]
pub struct AudioBridge {
    pub tx_cmd: Sender<AudioCmd>,     // Bevy_ecs -> audio thread
    pub rx_evt: Receiver<AudioEvent>, // audio thread -> Bevy_ecs
    pub handle: std::thread::JoinHandle<()>,
}

pub fn setup_audio(world: &mut World) {
    let (tx_cmd, rx_cmd) = unbounded::<AudioCmd>();
    let (tx_evt, rx_evt) = unbounded::<AudioEvent>();

    let handle = std::thread::spawn(move || audio_thread(rx_cmd, tx_evt));

    world.insert_resource(AudioBridge {
        tx_cmd,
        rx_evt,
        handle,
    });
    world.insert_resource(Events::<AudioEvent>::default());
}

pub fn shutdown_audio(world: &mut World) {
    if let Some(bridge) = world.remove_resource::<AudioBridge>() {
        let _ = bridge.tx_cmd.send(AudioCmd::Shutdown);
        let _ = bridge.handle.join();
    }
}
