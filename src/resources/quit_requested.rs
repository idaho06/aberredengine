//! Render-loop quit flag.
//!
//! Set by render-schedule systems (`sample_and_send_input` on logic-thread
//! disconnect, `pump_render_msgs` on `RenderMsg::Quit`) and read by the
//! render loop's `while` condition, which lives outside any schedule.
//! Phase 7f-1: replaces the loop-local `quit_requested: bool`.

use bevy_ecs::prelude::Resource;

/// When `true`, the render loop exits after the current schedule run.
#[derive(Resource, Default)]
pub struct QuitRequested(pub bool);
