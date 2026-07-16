//! Window resize event.
//!
//! [`WindowResizedEvent`] is triggered by the logic thread's per-tick
//! window-size mirror (see `engine_app.rs`) whenever the window dimensions
//! reported by the newest input sample differ from the previously recorded
//! [`crate::resources::windowsize::WindowSize`], and both new dimensions are
//! strictly positive.
//!
//! The very first tick does not trigger this event: the initial
//! `WindowSize` resource is inserted at startup with the real window size,
//! so there is nothing to diff against yet. Consumers that need the
//! startup size should read [`crate::resources::windowsize::WindowSize`]
//! directly instead of waiting for this event.

use bevy_ecs::prelude::*;

/// Triggered when the window's logical size changes.
///
/// Carries the *new* width/height (in the same units as
/// [`crate::resources::windowsize::WindowSize`]). Not triggered on the
/// first tick, and not triggered if either dimension is `<= 0`.
#[derive(Event, Debug, Clone, Copy)]
pub struct WindowResizedEvent {
    pub w: i32,
    pub h: i32,
}
