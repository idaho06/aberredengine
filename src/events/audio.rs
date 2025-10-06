//! Command and event types for the audio subsystem.
//!
//! This module defines the messages exchanged between the main ECS world and
//! the background audio thread. Commands are sent to control playback and
//! resource lifetime, while events are emitted back to report results and
//! state changes.
//!
//! See [`crate::resources::audio`] for the bridge that wires channels and
//! spawns the thread, and [`crate::systems::audio`] for the thread
//! implementation and polling systems.
#![allow(dead_code, unused_variables)]

use bevy_ecs::message::Message;

/// Commands sent *to* the audio thread
#[derive(Message, Debug, Clone)]
pub enum AudioCmd {
    /// Load a music stream from `path` and store it under `id`.
    LoadMusic { id: String, path: String },
    /// Unload a previously loaded music stream identified by `id`.
    UnloadMusic { id: String },
    /// Unload all music streams.
    UnloadAllMusic,
    /// Start playback of a music stream identified by `id`.
    /// If `looped` is true, the track restarts automatically when it ends.
    PlayMusic { id: String, looped: bool },
    /// Stop playback and reset the stream position for `id`.
    StopMusic { id: String },
    /// Pause playback for `id` (can be resumed).
    PauseMusic { id: String },
    /// Resume playback for a previously paused `id`.
    ResumeMusic { id: String },
    /// Set volume of a music stream `id` to `vol` in the `[0.0, 1.0]` range.
    VolumeMusic { id: String, vol: f32 },
    /// Load a sound effect from `path` and store it under `id`.
    LoadFx { id: String, path: String },
    /// Play a previously loaded sound effect `id` (one-shot).
    PlayFx { id: String },
    /// Unload a previously loaded sound effect `id`.
    UnloadFx { id: String },
    /// Unload all sound effects.
    UnloadAllFx,
    /// Terminate the audio thread after unloading all resources.
    Shutdown,
}

/// Events sent *back* from the audio thread
#[derive(Message, Debug, Clone)]
pub enum AudioMessage {
    /// Music with `id` successfully loaded.
    MusicLoaded { id: String },
    /// Music with `id` successfully unloaded.
    MusicUnloaded { id: String },
    /// All music resources have been unloaded.
    MusicUnloadedAll,
    /// Music with `id` failed to load with `error`.
    MusicLoadFailed { id: String, error: String },
    /// Playback of music `id` has started (including loop restarts).
    MusicPlayStarted { id: String },
    /// Playback of music `id` has been stopped or paused.
    MusicStopped { id: String },
    /// Non-looping music `id` reached the end of the stream.
    MusicFinished { id: String }, // reached end for non looping
    /// Volume of music `id` changed to `vol`.
    MusicVolumeChanged { id: String, vol: f32 },
    /// Sound effect with `id` successfully loaded.
    FxLoaded { id: String },
    /// Sound effect with `id` successfully unloaded.
    FxUnloaded { id: String },
    /// All sound effects have been unloaded.
    FxUnloadedAll,
    /// Sound effect with `id` failed to load with `error`.
    FxLoadFailed { id: String, error: String },
    /// One-shot sound effect `id` finished playing.
    FxFinished { id: String },
}
