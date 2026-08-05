//! Render-loop quit flag.
//!
//! Set by render-schedule systems (`sample_and_send_input` on logic-thread
//! disconnect, `pump_render_msgs` on `RenderMsg::Quit`) and read by the
//! render loop's `while` condition, which lives outside any schedule -- a
//! resource is needed here (rather than a loop-local `bool`) since it must
//! be writable from two different systems.

use bevy_ecs::prelude::Resource;

/// When `true`, the render loop exits after the current schedule run.
#[derive(Resource, Default)]
pub struct QuitRequested(pub bool);
