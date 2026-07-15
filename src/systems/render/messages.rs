//! Drains logic->render messages once per render frame.

use bevy_ecs::prelude::*;

use crate::events::render_assets::RenderAssetCmd;
use crate::events::render::switchfullscreen::SwitchFullScreenEvent;
use crate::protocol::endpoints::LogicBridge;
use crate::protocol::render_logic::RenderMsg;
use crate::resources::render::quit_requested::QuitRequested;

/// Drains logic->render messages once per frame: re-queues asset commands
/// into this world's `Messages<RenderAssetCmd>` for `process_render_asset_cmds`,
/// triggers `SwitchFullScreenEvent` (F10 decisions are resolved sim-side,
/// since key bindings live there, and arrive here as a decision to apply),
/// and sets `QuitRequested` on `RenderMsg::Quit`. EXCLUSIVE system
/// (`fn(&mut World)`, not a `SystemParam`-based one): the fullscreen
/// toggle's `world.trigger(...)` + `world.flush()` must apply synchronously
/// so it's visible to `render_system` later in this same schedule run -- a
/// regular system's `Commands::trigger` would only apply at a
/// scheduler-inserted sync point, not deterministically before the next
/// chained system.
pub fn pump_render_msgs(world: &mut World) {
    let msgs: Vec<RenderMsg> = {
        let bridge = world.resource::<LogicBridge>();
        bridge.rx_render.try_iter().collect()
    };

    let mut asset_cmds: Vec<RenderAssetCmd> = Vec::new();
    let mut toggle_fullscreen = false;
    for msg in msgs {
        match msg {
            RenderMsg::Asset(cmd) => asset_cmds.push(cmd),
            RenderMsg::ToggleFullscreen => toggle_fullscreen = true,
            RenderMsg::Quit => world.resource_mut::<QuitRequested>().0 = true,
        }
    }
    if toggle_fullscreen {
        world.trigger(SwitchFullScreenEvent {});
        world.flush();
    }
    if !asset_cmds.is_empty() {
        world
            .resource_mut::<Messages<RenderAssetCmd>>()
            .write_batch(asset_cmds);
    }
}
