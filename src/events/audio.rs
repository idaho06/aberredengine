#![allow(dead_code, unused_variables)]

use bevy_ecs::message::Message;

/// Commands set *to* the audio thread
#[derive(Debug)]
pub enum AudioCmd {
    Load { id: String, path: String },
    Unload { id: String },
    UnloadAll,
    Play { id: String, looped: bool },
    Stop { id: String },
    Pause { id: String },
    Resume { id: String },
    Volume { id: String, vol: f32 },
    Shutdown,
}

/// Events sent *back* from the audio thread
#[derive(Message, Debug, Clone)]
pub enum AudioMessage {
    Loaded { id: String },
    Unloaded { id: String },
    UnloadedAll,
    LoadFailed { id: String, error: String },
    PlayStarted { id: String },
    Stopped { id: String },
    Finished { id: String }, // reached end for non looping
    VolumeChanged { id: String, vol: f32 },
}
