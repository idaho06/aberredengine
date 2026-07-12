//! Channel message types exchanged between the render (main) thread and the
//! logic thread (Phase 5e of the Option B render/logic split).
//!
//! These are plain crossbeam-channel payloads, not bevy `Message`s — they
//! never live in a `Messages<T>` queue. Both enums are fully `Send` (asserted
//! at compile time below), which is what structurally guarantees the two
//! `World`s never exchange NonSend data.
//!
//! See `docs/plans/phase5e-thread-cutover.md` for the full channel spec and
//! `src/protocol/endpoints.rs` for the bridge resources holding the
//! endpoints.

use crate::events::render_assets::RenderAssetCmd;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::drawable_snapshot::DrawableSnapshot;
use crate::resources::fontmetrics::FontMetrics;
use crate::resources::imgui_bridge::ImguiCaptureState;
use crate::resources::input_bindings::InputBindings;
use crate::resources::rawinput::RawInputSnapshot;
use crate::resources::signal_intents::SignalIntent;

/// Render thread -> logic thread messages.
#[derive(Debug, Clone)]
pub enum LogicMsg {
    /// One raw input sample per render frame. Carries the current OS window
    /// dimensions plus that render frame's real delta so the logic world's
    /// `WindowSize` mirror stays fresh and the `PRESENT` schedule can observe
    /// the same frame delta the render loop just measured. `capture` is the
    /// PREVIOUS render frame's imgui capture state (`ImguiBridge::render`
    /// runs after this message is sent each frame, so it's one frame behind,
    /// same latency class as `SignalIntents` -- Phase 6e); used by
    /// `apply_input_snapshot` to mask gameplay input while the debug overlay
    /// has focus.
    Input {
        snapshot: RawInputSnapshot,
        frame_dt: f32,
        window_w: i32,
        window_h: i32,
        capture: ImguiCaptureState,
    },
    /// Sent after `apply_gameconfig_changes` recreates the render target;
    /// the logic world mirrors it (`camera_follow_system` and input math
    /// read `ScreenSize`).
    ScreenSize { w: i32, h: i32 },
    /// CPU-side glyph metrics extracted after a render-side font load.
    /// Replaces Phase 5c's direct `ResMut<FontMetricsStore>` population in
    /// `process_render_asset_cmds`.
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
    /// Imgui debug-checkbox edits (render-owned `DebugOverlayConfig`); the
    /// logic-side mirror gates `build_drawable_snapshot`'s per-entity
    /// `Signals` clone. Sent on change only.
    OverlayConfig(DebugOverlayConfig),
    /// `SignalIntents` drained from the render world after `render_system`
    /// each frame (queued by `GuiCallback`, Phase 5d); applied to
    /// `WorldSignals` by `apply_signal_intents` at the top of the next logic
    /// FIXED substep (Phase 6d; was "the next VARIABLE pass" pre-6d).
    SignalIntents(Vec<SignalIntent>),
    /// The window is closing; the logic thread breaks its loop, shuts down
    /// audio, and joins.
    Shutdown,
}

/// Logic thread -> render thread messages.
#[derive(Debug, Clone)]
pub enum RenderMsg {
    /// One full drawable snapshot per logic `PRESENT` pass. The render loop
    /// `try_iter()`s and keeps only the newest (no interpolation).
    Snapshot(Box<DrawableSnapshot>),
    /// A GL asset load/upload/remove request forwarded from the logic
    /// world's `Messages<RenderAssetCmd>` queue by
    /// `forward_render_asset_cmds`; re-queued into the render world's own
    /// `Messages<RenderAssetCmd>` and drained by `process_render_asset_cmds`.
    Asset(RenderAssetCmd),
    /// Input bindings changed logic-side (Lua `rebind_action`/`add_binding`
    /// or `GameCtx.input_bindings`); the render side updates the mirror its
    /// `sample_input_snapshot` call reads. Sent on change only.
    Bindings(InputBindings),
    /// Game-initiated quit (`quit_game`); the render loop breaks, same path
    /// as a window close.
    Quit,
}

/// Both message enums must stay fully `Send` — this is the structural
/// guarantee that no NonSend (GL/Lua) data ever crosses the thread boundary.
const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<LogicMsg>();
    assert_send::<RenderMsg>();
};
