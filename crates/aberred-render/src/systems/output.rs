//! Diffs render-owned mirrors against their previous-frame values and
//! ships changes back to the logic thread once per render frame.

use bevy_ecs::prelude::*;

use aberred_core::protocol::endpoints::LogicBridge;
use aberred_core::protocol::render_logic::LogicMsg;
use aberred_core::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::imgui_bridge::ImguiBridge;
use crate::resources::pending_imgui_capture::PendingImguiCapture;
use aberred_core::resources::screensize::ScreenSize;
use aberred_core::resources::signal_intents::SignalIntents;

/// Diffs the render-owned mirrors (`ScreenSize`, `DebugOverlayConfig`)
/// against their previous-frame values and sends `LogicMsg`s on change;
/// drains `SignalIntents` queued by this frame's `GuiCallback`; refreshes
/// `PendingImguiCapture` for `sample_and_send_input`'s NEXT frame read (a
/// one-frame imgui-capture lag). The `Local<Option<T>>`s hold the
/// `last_screen_size`/`last_overlay_config` values -- both are read AND
/// written by this same system across repeated `schedule.run()` calls,
/// which is exactly what `Local<T>` is for; wrapped in `Option` so this
/// compiles without requiring `ScreenSize`/`DebugOverlayConfig` to
/// implement `Default` (neither does today) -- `Option<T>: Default` holds
/// unconditionally. Consequence: frame 1 always sees `None != Some(current)`
/// and sends once even if nothing changed since startup -- harmless, the
/// logic thread already expects an initial `ScreenSize`/`OverlayConfig`
/// update.
#[allow(clippy::too_many_arguments)]
pub fn send_render_mirrors(
    screen_size: Res<ScreenSize>,
    mut last_screen_size: Local<Option<ScreenSize>>,
    overlay_config: Res<DebugOverlayConfig>,
    mut last_overlay_config: Local<Option<DebugOverlayConfig>>,
    mut intents: ResMut<SignalIntents>,
    bridge: Res<LogicBridge>,
    imgui: NonSend<ImguiBridge>,
    mut pending_capture: ResMut<PendingImguiCapture>,
) {
    if *last_screen_size != Some(*screen_size) {
        *last_screen_size = Some(*screen_size);
        let _ = bridge.tx_logic.send(LogicMsg::ScreenSize {
            w: screen_size.w,
            h: screen_size.h,
        });
    }
    if last_overlay_config.as_ref() != Some(&*overlay_config) {
        *last_overlay_config = Some(overlay_config.clone());
        let _ = bridge
            .tx_logic
            .send(LogicMsg::OverlayConfig(overlay_config.clone()));
    }
    let intents_taken = std::mem::take(&mut intents.0);
    if !intents_taken.is_empty() {
        let _ = bridge.tx_logic.send(LogicMsg::SignalIntents(intents_taken));
    }
    pending_capture.0 = imgui.capture_state();
}
