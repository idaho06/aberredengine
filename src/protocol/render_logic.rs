//! Channel message types exchanged between the render (main) thread and the
//! logic thread.
//!
//! These are plain crossbeam-channel payloads, not bevy `Message`s — they
//! never live in a `Messages<T>` queue. Both enums are fully `Send` (asserted
//! at compile time below), which is what structurally guarantees the two
//! `World`s never exchange NonSend data.
//!
//! See `src/protocol/endpoints.rs` for the bridge resources holding the
//! endpoints.

use crate::events::render_assets::RenderAssetCmd;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontmetrics::FontMetrics;
use crate::resources::signal_intents::SignalIntent;

/// Render thread -> logic thread messages.
///
/// Raw input travels separately, as [`crate::protocol::raw_input::InputSample`]
/// over its own dedicated bounded channel (`LogicBridge::tx_input` /
/// `LogicInit::rx_input`), apart from this unbounded channel — so a stalled
/// sim drops the oldest-queued input samples instead of growing an unbounded
/// backlog of every other message kind here too.
#[derive(Debug, Clone)]
pub enum LogicMsg {
    /// Sent after `apply_gameconfig_changes` recreates the render target;
    /// the logic world mirrors it (`camera_follow_system` and input math
    /// read `ScreenSize`).
    ScreenSize { w: i32, h: i32 },
    /// CPU-side glyph metrics extracted after a render-side font load,
    /// sent by `process_render_asset_cmds`.
    FontLoaded { key: String, metrics: FontMetrics },
    /// Pixel dimensions of a texture the render side just loaded/uploaded
    /// (`Texture`, `TilemapTexture`, and `RasterizeText` arms). Feeds the
    /// logic-side `TextureDimsStore` used by `animation`'s multi-row frame
    /// wrap.
    TextureLoaded {
        key: String,
        width: i32,
        height: i32,
    },
    /// Sent after `RenderAssetCmd::RemoveTexture` drops the GPU handle;
    /// the logic side prunes the now-stale entry from `TextureDimsStore`.
    TextureRemoved { key: String },
    /// Sent after `RenderAssetCmd::RemoveFont` drops the GPU handle; the
    /// logic side prunes the now-stale entry from `FontMetricsStore`.
    FontRemoved { key: String },
    /// Sent after `RenderAssetCmd::RenameTexture` moves a texture to a new
    /// key in place; the logic side moves the matching `TextureDimsStore`
    /// entry so the CPU-side dims mirror follows the rename.
    TextureRenamed { old_key: String, new_key: String },
    /// Sent after `RenderAssetCmd::RenameFont` moves a font to a new key in
    /// place; the logic side moves the matching `FontMetricsStore` entry so
    /// the CPU-side metrics mirror follows the rename.
    FontRenamed { old_key: String, new_key: String },
    /// Imgui debug-checkbox edits (render-owned `DebugOverlayConfig`); the
    /// logic-side mirror gates `build_drawable_snapshot`'s per-entity
    /// `Signals` clone. Sent on change only.
    OverlayConfig(DebugOverlayConfig),
    /// `SignalIntents` drained from the render world after `render_system`
    /// each frame (queued by `GuiCallback`); applied to `WorldSignals` by
    /// `apply_signal_intents` at the top of the next sim tick.
    SignalIntents(Vec<SignalIntent>),
    /// The window is closing; the logic thread breaks its loop, shuts down
    /// audio, and joins.
    Shutdown,
}

/// Logic thread -> render thread messages.
///
/// `DrawableSnapshot` travels separately, published through the
/// `triple_buffer` transport in [`crate::protocol::snapshot`], latest-wins
/// with no queue growth.
#[derive(Debug, Clone)]
pub enum RenderMsg {
    /// A GL asset load/upload/remove request forwarded from the logic
    /// world's `Messages<RenderAssetCmd>` queue by
    /// `forward_render_asset_cmds`; re-queued into the render world's own
    /// `Messages<RenderAssetCmd>` and drained by `process_render_asset_cmds`.
    Asset(RenderAssetCmd),
    /// F10's edge fired sim-side (`resolve_input_backlog`, where input
    /// bindings are resolved); the render loop triggers
    /// `SwitchFullScreenEvent` on its own world in response
    /// (`switch_fullscreen_observer` and the `FullScreen` resource live
    /// there — the window only exists render-side). Costs one sim tick of
    /// latency (~4ms at the default 240Hz `sim_hz`), imperceptible for a
    /// manual toggle.
    ToggleFullscreen,
    /// Game-initiated quit (`quit_game`); the render loop breaks, same path
    /// as a window close.
    Quit,
}

/// Both message enums must stay fully `Send` — this is the structural
/// guarantee that no NonSend (GL/Lua) data ever crosses the thread boundary.
/// [`crate::protocol::raw_input::InputSample`] is asserted alongside them
/// since it crosses the same thread boundary, just on its own channel.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<LogicMsg>();
    assert_send::<RenderMsg>();
    assert_send::<crate::protocol::raw_input::InputSample>();
};
