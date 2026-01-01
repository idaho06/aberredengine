//! Game configuration change detection system.
//!
//! Monitors [`GameConfig`] for changes and applies settings to the window,
//! render target, and screen size resources.
//!
//! On initial insertion, loads configuration from the INI file. On subsequent
//! changes, applies the new settings to the running game.

use crate::events::switchfullscreen::SwitchFullScreenEvent;
use crate::resources::fullscreen::FullScreen;
use crate::resources::gameconfig::GameConfig;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::screensize::ScreenSize;
use crate::resources::windowsize::WindowSize;
use bevy_ecs::prelude::*;
use raylib::ffi;
//use std::time::Duration;

/// System that applies game configuration changes.
///
/// This system detects when [`GameConfig`] is added or modified and:
/// 1. On first addition: loads settings from the config file
/// 2. On any change: applies render size, window size, and FPS settings
///
/// # Resource Dependencies
/// - `GameConfig` (optional, mutable) - the configuration to monitor
/// - `RaylibHandle` (non-send, mutable) - for window operations
/// - `RaylibThread` (non-send) - required for render texture recreation
/// - `RenderTarget` (non-send, mutable) - for render resolution changes
/// - `ScreenSize` (mutable) - updated to match render resolution
pub fn apply_gameconfig_changes(
    maybe_config: Option<Res<GameConfig>>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    mut render_target: NonSendMut<RenderTarget>,
    mut screen_size: ResMut<ScreenSize>,
    mut _window_size: ResMut<WindowSize>,
    fullscreen: Option<Res<FullScreen>>,
    mut commands: Commands,
) {
    let Some(config) = maybe_config else {
        return;
    };

    // On first insertion, load configuration from file
    /*     if config.is_added() {
           eprintln!("GameConfig added, loading from file...");
           if let Err(e) = config.bypass_change_detection().load_from_file() {
               eprintln!("Config file not found or invalid, using defaults: {}", e);
           }
           // Fall through to apply loaded (or default) values
       }
    */
    // Apply changes when config is added or modified
    if config.is_changed() || config.is_added() {
        // Apply render size if different from current
        if render_target.game_width != config.render_width
            || render_target.game_height != config.render_height
        {
            eprintln!(
                "Resizing render target: {}x{} -> {}x{}",
                render_target.game_width,
                render_target.game_height,
                config.render_width,
                config.render_height
            );
            if let Err(e) =
                render_target.recreate(&mut rl, &th, config.render_width, config.render_height)
            {
                eprintln!("Failed to resize render target: {}", e);
            } else {
                screen_size.w = config.render_width as i32;
                screen_size.h = config.render_height as i32;
            }
        }

        // Synchronize fullscreen state between config and window
        let is_fullscreen = fullscreen.is_some();
        if config.fullscreen != is_fullscreen {
            // Config and window state don't match - fire event to toggle
            eprintln!(
                "Fullscreen mismatch: config={}, window={} - triggering toggle",
                config.fullscreen, is_fullscreen
            );
            commands.trigger(SwitchFullScreenEvent {});
        }

        // Apply window size if not fullscreen in the config
        /*         if !config.fullscreen {
                   let (w, h) = config.window_size();
                   let current_w = rl.get_screen_width();
                   let current_h = rl.get_screen_height();
                   if current_w != w as i32 || current_h != h as i32 {
                       eprintln!(
                           "Resizing window: {}x{} -> {}x{}",
                           current_w, current_h, w, h
                       );
                       rl.set_window_size(w as i32, h as i32);
                       // update window size in resource WindowSize if exists
                       window_size.w = w as i32;
                       window_size.h = h as i32;
                   }
               }
        */
        // TODO: This currently does not handle switching to fullscreen mode. Raylib bug??

        // Apply vsync setting
        unsafe {
            if config.vsync {
                ffi::SetWindowState(ffi::ConfigFlags::FLAG_VSYNC_HINT as u32);
                eprintln!("VSync enabled");
            } else {
                ffi::ClearWindowState(ffi::ConfigFlags::FLAG_VSYNC_HINT as u32);
                eprintln!("VSync disabled");
            }
        }

        // Apply target FPS
        rl.set_target_fps(config.target_fps);

        eprintln!("GameConfig changes applied.");
    }
    // clean up change detection flag
    // config.bypass_change_detection();
}
