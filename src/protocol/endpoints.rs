//! ECS resources holding the channel endpoints between the render (main)
//! thread and the logic thread (Phase 5e of the Option B render/logic split).
//!
//! Mirrors this module's own [`AudioBridge`] shape (audio bridge below): the render
//! world owns the [`LogicBridge`] (sender + receiver + join handle), the
//! logic world owns the [`RenderTx`] newtype endpoint (its receiver is held
//! directly by the logic thread's event loop, outside the world). All
//! endpoints are crossbeam channels (`Send`), so both resources are plain
//! `Resource`s, never NonSend.
//!
//! Unlike `setup_audio`, thread spawning is not done here: the logic thread
//! needs the whole `LogicInit` payload built by `EngineBuilder::try_run`
//! (hooks, scenes, config), so `engine_app.rs` wires the channels and spawns
//! the thread itself. Teardown does live here ([`shutdown_logic`], the
//! `shutdown_audio` counterpart).

use crate::protocol::raw_input::InputSample;
use crate::protocol::render_logic::{LogicMsg, RenderMsg};
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender};

/// Render-world bridge to the logic thread.
#[derive(Resource)]
pub struct LogicBridge {
    /// Sender for [`LogicMsg`] (render -> logic).
    pub tx_logic: Sender<LogicMsg>,
    /// Sender for [`InputSample`] (render -> logic), on its own bounded
    /// channel (Phase 7d) separate from `tx_logic`'s unbounded one — a stalled
    /// sim drops the oldest-queued samples instead of growing an unbounded
    /// backlog.
    pub tx_input: Sender<InputSample>,
    /// Receiver for [`RenderMsg`] (logic -> render).
    pub rx_render: Receiver<RenderMsg>,
    /// Join handle for the logic thread; joined during shutdown, after
    /// sending [`LogicMsg::Shutdown`].
    pub handle: std::thread::JoinHandle<()>,
}

/// Render-world sender newtype for systems that only need to send
/// [`LogicMsg`]s (e.g. `process_render_asset_cmds`'s `FontLoaded`/
/// `TextureLoaded` notifications) without borrowing the whole bridge.
#[derive(Resource, Clone)]
pub struct LogicTx(pub Sender<LogicMsg>);

/// Logic-world sender endpoint (logic -> render).
#[derive(Resource, Clone)]
pub struct RenderTx(pub Sender<RenderMsg>);

/// Gracefully stop the logic thread and join it (mirrors
/// [`shutdown_audio`](crate::protocol::endpoints::shutdown_audio)).
///
/// If the bridge resource exists, sends [`LogicMsg::Shutdown`], waits for the
/// thread to exit (it runs `shutdown_audio` and drops `LuaRuntime` on its own
/// thread first), and removes the resource from the world.
pub fn shutdown_logic(world: &mut World) {
    if let Some(bridge) = world.remove_resource::<LogicBridge>() {
        shutdown_logic_bridge(bridge);
    }
}

/// Send [`LogicMsg::Shutdown`] and join the logic thread from an owned
/// [`LogicBridge`] that hasn't (yet) been inserted as a world resource — e.g.
/// a render-world setup step that fails after the logic thread was already
/// spawned but before the bridge was handed to the `World`. Without this,
/// dropping the bridge leaks the thread: `JoinHandle::drop` detaches rather
/// than joining, so the thread would keep running forever.
pub fn shutdown_logic_bridge(bridge: LogicBridge) {
    let _ = bridge.tx_logic.send(LogicMsg::Shutdown);
    if bridge.handle.join().is_err() {
        log::error!("Logic thread panicked during shutdown");
    }
}

// --- Audio thread bridge -----------------------------------------------
//
// ECS resources that bridge the main thread with the background audio
// thread. Use [`setup_audio`] once during initialization to spawn the audio
// thread and insert the [`AudioBridge`] and `Messages<AudioMessage>`
// resources. Call [`shutdown_audio`] during teardown to gracefully stop the
// thread and free audio resources.

use crate::protocol::audio::{AudioCmd, AudioMessage};
use crate::systems::audio::audio_thread;
use crossbeam_channel::unbounded;

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
/// - Spawns the background thread running [`audio_thread`], paced at
///   `audio_hz` (Phase 7b; read once here, at spawn time -- a runtime
///   change to `GameConfig::audio_hz` after startup has no effect).
/// - Inserts [`AudioBridge`] and initializes `Messages<AudioMessage>` so that
///   systems can send commands and poll for events.
pub fn setup_audio(world: &mut World, audio_hz: f64) {
    let (tx_cmd, rx_cmd) = unbounded::<AudioCmd>();
    let (tx_msg, rx_msg) = unbounded::<AudioMessage>();

    let handle = std::thread::spawn(move || audio_thread(rx_cmd, tx_msg, audio_hz));

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
