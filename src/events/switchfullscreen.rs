//! Fullscreen toggle event and observer.
//!
//! Pressing **F10** triggers [`SwitchFullScreenEvent`], which is handled by
//! [`switch_fullscreen_observer`]. The observer toggles the window between
//! fullscreen and windowed mode, using the [`FullScreen`] marker resource to
//! track the current state.

use crate::resources::fullscreen::FullScreen;
use crate::resources::gameconfig::GameConfig;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use log::{debug, info};

/// Event triggered to toggle fullscreen mode.
///
/// Fired by the input system when the fullscreen key (F10) is pressed.
/// The [`switch_fullscreen_observer`] handles this event.
#[derive(Event, Debug, Clone, Copy)]
pub struct SwitchFullScreenEvent {}

/// Observer that toggles fullscreen mode when [`SwitchFullScreenEvent`] fires.
///
/// - If [`FullScreen`] resource exists: removes it and exits fullscreen.
/// - If [`FullScreen`] resource is absent: inserts it and enters fullscreen,
///   resizing the window to match the current monitor dimensions.
pub fn switch_fullscreen_observer(
    _trigger: On<SwitchFullScreenEvent>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    mut commands: Commands,
    fullscreen: Option<Res<FullScreen>>,
    config: Res<GameConfig>,
) {
    debug!("SwitchFullScreenEvent triggered");
    if fullscreen.is_some() {
        commands.remove_resource::<FullScreen>();
        rl.toggle_borderless_windowed();
        let (w, h) = config.window_size();
        rl.set_window_size(w as i32, h as i32);
        info!("Full screen disabled");
    } else {
        commands.insert_resource(FullScreen {});
        rl.toggle_borderless_windowed();
        info!("Full screen enabled");
    }
}
