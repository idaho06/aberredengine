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
use log::{info, error};
use raylib::ffi;

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
    //th: NonSend<raylib::RaylibThread>,
    mut commands: Commands,
    fullscreen: Option<Res<FullScreen>>,
    config: Res<GameConfig>,
) {
    // This observer is triggered when a SwitchFullScreenEvent is fired.
    // It toggles the FullScreen resource.
    info!("SwitchFullScreenEvent triggered");
    if fullscreen.is_some() {
        // If it exists, we remove it
        commands.remove_resource::<FullScreen>();

        if rl.is_window_fullscreen() {
            #[cfg(target_os = "windows")]
            {
                //rl.toggle_borderless_windowed();
                //rl.restore_window();
                rl.toggle_fullscreen();
                let (w, h) = config.window_size();
                rl.set_window_size(w as i32, h as i32);
                rl.restore_window();
            }
            #[cfg(not(target_os = "windows"))]
            {
                rl.toggle_fullscreen();
                let (w, h) = config.window_size();
                rl.set_window_size(w as i32, h as i32);
                rl.restore_window();
            }

            if !rl.is_window_fullscreen() {
                info!("Full screen disabled");
            } else {
                error!("Failed to disable full screen");
            }
        }
    } else {
        info!("Entering full screen mode");
        commands.insert_resource(FullScreen {});

        if !rl.is_window_fullscreen() {
            // get monitor dimensions
            //#[cfg(not(target_os = "windows"))]
            {
                //rl.set_window_position(0, 0);
                rl.maximize_window();
                let monitor: i32 = unsafe { ffi::GetCurrentMonitor() };
                let monitor_width = unsafe { ffi::GetMonitorWidth(monitor) };
                let monitor_height = unsafe { ffi::GetMonitorHeight(monitor) };
                info!("Monitor dimensions: {}x{}", monitor_width, monitor_height);
                // resize window to monitor dimensions
                rl.set_window_size(monitor_width, monitor_height);
            }

            #[cfg(target_os = "windows")]
            {
                //rl.toggle_borderless_windowed();
                //rl.maximize_window();
                rl.toggle_fullscreen();
            }
            #[cfg(not(target_os = "windows"))]
            {
                rl.maximize_window();
                rl.toggle_fullscreen();
            }

            if rl.is_window_fullscreen() {
                info!("Full screen enabled");
            } else {
                error!("Failed to enable full screen");
            }
        }
    }
}
