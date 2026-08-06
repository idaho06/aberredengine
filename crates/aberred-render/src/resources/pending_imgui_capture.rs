//! One-frame-lag imgui capture state, pending send to the logic thread.
//!
//! Written by `send_render_mirrors` from `ImguiBridge::capture_state()`
//! after `render_system` runs each frame; read by `sample_and_send_input`
//! at the start of the *next* frame's schedule run. This resource is the
//! cross-system carrier for that one-frame lag -- a `Local<T>` would not
//! work here since the writer and reader are different systems.

use bevy_ecs::prelude::Resource;

use aberred_core::protocol::raw_input::ImguiCaptureState;

/// Imgui capture state captured after the last render pass, awaiting
/// send on the next frame's `InputSample`.
#[derive(Resource, Default)]
pub struct PendingImguiCapture(pub ImguiCaptureState);
