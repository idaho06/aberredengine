#![allow(dead_code, unused_variables)]

use bevy_ecs::message::Message;

/// Commands set *to* the audio thread
#[derive(Debug)]
pub enum AudioCmd {
    LoadMusic { id: String, path: String },
    UnloadMusic { id: String },
    UnloadAllMusic,
    PlayMusic { id: String, looped: bool },
    StopMusic { id: String },
    PauseMusic { id: String },
    ResumeMusic { id: String },
    VolumeMusic { id: String, vol: f32 },
    LoadFx { id: String, path: String },
    PlayFx { id: String },
    UnloadFx { id: String },
    UnloadAllFx,
    Shutdown,
}

/// Events sent *back* from the audio thread
#[derive(Message, Debug, Clone)]
pub enum AudioMessage {
    MusicLoaded { id: String },
    MusicUnloaded { id: String },
    MusicUnloadedAll,
    MusicLoadFailed { id: String, error: String },
    MusicPlayStarted { id: String },
    MusicStopped { id: String },
    MusicFinished { id: String }, // reached end for non looping
    MusicVolumeChanged { id: String, vol: f32 },
    FxLoaded { id: String },
    FxUnloaded { id: String },
    FxUnloadedAll,
    FxLoadFailed { id: String, error: String },
    FxFinished { id: String },
}
