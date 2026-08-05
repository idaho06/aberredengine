//! Game configuration change detection system.
//!
//! Monitors the [`GameConfig`] mirrored into [`RenderGameConfig`] for changes
//! and applies settings to the window, render target, and screen size
//! resources.

use crate::events::render::switchfullscreen::SwitchFullScreenEvent;
use aberred_core::resources::gameconfig::GameConfig;
use crate::resources::render::fullscreen::FullScreen;
use crate::resources::render::mirrors::RenderGameConfig;
use crate::resources::render::rendertarget::RenderTarget;
use aberred_core::resources::screensize::ScreenSize;
use bevy_ecs::prelude::*;
use log::{debug, error};
use raylib::ffi;

/// System that applies game configuration changes.
///
/// Reads config from [`RenderGameConfig`], not `Res<GameConfig>` -- the
/// render thread has no live `GameConfig` resource of its own; received
/// snapshots are its only config source. Runs on the render schedule after
/// `receive_snapshot` and before `render_system`, so a config change made
/// this frame (Lua command) is applied before this frame renders.
///
/// Change detection is a value diff against the last-applied config
/// (`Local<Option<GameConfig>>`) -- a mirrored snapshot read has no
/// `Res::is_changed()` to lean on, so this system tracks its own baseline
/// instead. The gate matters: F10's `switch_fullscreen_observer` toggles
/// the window and the `FullScreen` resource *without* writing
/// `GameConfig.fullscreen`, so an ungated `config.fullscreen !=
/// fullscreen.is_some()` comparison would re-trigger the fullscreen toggle
/// every frame after F10, fighting the user. With the value diff, a config
/// that hasn't changed since last application is never re-examined.
///
/// # Resource Dependencies
/// - `RenderGameConfig` (read) - source of the config to apply
/// - `RaylibHandle` (non-send, mutable) - for window operations
/// - `RaylibThread` (non-send) - required for render texture recreation
/// - `RenderTarget` (non-send, mutable) - for render resolution changes
/// - `ScreenSize` (mutable) - updated to match render resolution
pub fn apply_gameconfig_changes(
    render_game_config: Res<RenderGameConfig>,
    mut raylib: super::RaylibAccess,
    mut render_target: NonSendMut<RenderTarget>,
    mut screen_size: ResMut<ScreenSize>,
    fullscreen: Option<Res<FullScreen>>,
    mut commands: Commands,
    mut last_applied: Local<Option<GameConfig>>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    let config = &render_game_config.0;

    // Apply changes on first run (mirrors the old `is_added()` path) or when
    // the config differs from what was last applied.
    if last_applied.as_ref() != Some(config) {
        // Apply render size if different from current
        if render_target.game_width != config.render_width
            || render_target.game_height != config.render_height
        {
            debug!(
                "Resizing render target: {}x{} -> {}x{}",
                render_target.game_width,
                render_target.game_height,
                config.render_width,
                config.render_height
            );
            if let Err(e) =
                render_target.recreate(rl, th, config.render_width, config.render_height)
            {
                error!("Failed to resize render target: {}", e);
            } else {
                screen_size.w = config.render_width as i32;
                screen_size.h = config.render_height as i32;
            }
        }

        // Apply render target filter if changed
        if config.render_target_filter != render_target.filter {
            render_target.set_filter(config.render_target_filter);
        }

        // Synchronize fullscreen state between config and window
        let is_fullscreen = fullscreen.is_some();
        if config.fullscreen != is_fullscreen {
            // Config and window state don't match - fire event to toggle
            debug!(
                "Fullscreen mismatch: config={}, window={} - triggering toggle",
                config.fullscreen, is_fullscreen
            );
            commands.trigger(SwitchFullScreenEvent {});
        }

        // Note: resizing the OS window to match config.window_size() when not
        // fullscreen is not implemented. WindowSize is refreshed every frame
        // from the actual window size regardless (see engine_app.rs).

        // Apply window title if changed. Compared against `last_applied`
        // (not a live window query — raylib has no GetWindowTitle) so an
        // unchanged title is never redundantly re-applied.
        if last_applied.as_ref().map(|c| &c.window_title) != Some(&config.window_title) {
            rl.set_window_title(th, &config.window_title);
            debug!("Window title set to '{}'", config.window_title);
        }

        // Apply vsync setting only if it differs from the current window state
        let vsync_flag = ffi::ConfigFlags::FLAG_VSYNC_HINT as u32;
        let vsync_active = unsafe { ffi::IsWindowState(vsync_flag) };
        if config.vsync != vsync_active {
            unsafe {
                if config.vsync {
                    ffi::SetWindowState(vsync_flag);
                    debug!("VSync enabled");
                } else {
                    ffi::ClearWindowState(vsync_flag);
                    debug!("VSync disabled");
                }
            }
        }

        // Apply target FPS
        rl.set_target_fps(config.target_fps);

        *last_applied = Some(config.clone());
        debug!("GameConfig changes applied.");
    }
}
