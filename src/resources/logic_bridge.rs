//! ECS resources holding the channel endpoints between the render (main)
//! thread and the logic thread (Phase 5e of the Option B render/logic split).
//!
//! Mirrors [`crate::resources::audio`]'s `AudioBridge` shape: the render
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

use crate::events::logic_bridge::{LogicMsg, RenderMsg};
use bevy_ecs::prelude::*;
use crossbeam_channel::{Receiver, Sender};

/// Render-world bridge to the logic thread.
#[derive(Resource)]
pub struct LogicBridge {
    /// Sender for [`LogicMsg`] (render -> logic).
    pub tx_logic: Sender<LogicMsg>,
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
/// [`shutdown_audio`](crate::resources::audio::shutdown_audio)).
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
