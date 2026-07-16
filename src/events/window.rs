//! Window resize event.
//!
//! [`WindowResizedEvent`] is triggered by the sim-schedule system
//! `detect_window_resize` (`crate::systems::window`) — see that system's
//! doc comment for the trigger conditions and mirroring/ordering mechanics.

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
