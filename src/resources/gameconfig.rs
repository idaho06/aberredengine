//! Game configuration resource.
//!
//! Manages game settings loaded from an INI configuration file. Provides
//! defaults for safe startup and methods to load/save configuration.
//!
//! # Configuration File Format
//!
//! ```ini
//! [render]
//! width = 640
//! height = 360
//!
//! [window]
//! width = 1280
//! height = 720
//! fullscreen = false
//! vsync = true
//! target_fps = 120
//! ```

use bevy_ecs::prelude::*;
use configparser::ini::Ini;
use log::info;
use std::path::PathBuf;

/// Default safe values for startup
const DEFAULT_RENDER_WIDTH: u32 = 640;
const DEFAULT_RENDER_HEIGHT: u32 = 360;
const DEFAULT_WINDOW_WIDTH: u32 = 1280;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
const DEFAULT_TARGET_FPS: u32 = 120;
const DEFAULT_VSYNC: bool = true;
const DEFAULT_FULLSCREEN: bool = false;
const DEFAULT_CONFIG_PATH: &str = "./config.ini";

/// Game configuration resource.
///
/// Stores render resolution, window settings, and other configurable options.
/// On first insertion into the ECS world, the [`apply_gameconfig_changes`]
/// system will attempt to load values from the configuration file.
///
/// [`apply_gameconfig_changes`]: crate::systems::gameconfig::apply_gameconfig_changes
#[derive(Resource, Debug, Clone)]
pub struct GameConfig {
    /// Internal render width in pixels.
    pub render_width: u32,
    /// Internal render height in pixels.
    pub render_height: u32,
    /// Window width in pixels.
    pub window_width: u32,
    /// Window height in pixels.
    pub window_height: u32,
    /// Target frames per second.
    pub target_fps: u32,
    /// Enable vertical sync.
    pub vsync: bool,
    /// Start in fullscreen mode.
    pub fullscreen: bool,
    /// Path to the configuration file.
    pub config_path: PathBuf,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl GameConfig {
    /// Create a new configuration with safe default values.
    pub fn new() -> Self {
        Self {
            render_width: DEFAULT_RENDER_WIDTH,
            render_height: DEFAULT_RENDER_HEIGHT,
            window_width: DEFAULT_WINDOW_WIDTH,
            window_height: DEFAULT_WINDOW_HEIGHT,
            target_fps: DEFAULT_TARGET_FPS,
            vsync: DEFAULT_VSYNC,
            fullscreen: DEFAULT_FULLSCREEN,
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        }
    }

    /// Create a new configuration with a custom config file path.
    #[allow(dead_code)]
    pub fn with_path(path: impl Into<PathBuf>) -> Self {
        Self {
            config_path: path.into(),
            ..Self::new()
        }
    }

    /// Load configuration from the INI file.
    ///
    /// Missing values retain their current (default) values.
    /// Returns an error if the file cannot be read or parsed.
    pub fn load_from_file(&mut self) -> Result<(), String> {
        let mut config = Ini::new();
        config
            .load(&self.config_path)
            .map_err(|e| format!("Failed to load config file: {}", e))?;

        // [render] section
        if let Some(width) = config.getuint("render", "width").ok().flatten() {
            self.render_width = width as u32;
        }
        if let Some(height) = config.getuint("render", "height").ok().flatten() {
            self.render_height = height as u32;
        }

        // [window] section
        if let Some(width) = config.getuint("window", "width").ok().flatten() {
            self.window_width = width as u32;
        }
        if let Some(height) = config.getuint("window", "height").ok().flatten() {
            self.window_height = height as u32;
        }
        if let Some(fps) = config.getuint("window", "target_fps").ok().flatten() {
            self.target_fps = fps as u32;
        }
        if let Some(vsync) = config.getbool("window", "vsync").ok().flatten() {
            self.vsync = vsync;
        }
        if let Some(fullscreen) = config.getbool("window", "fullscreen").ok().flatten() {
            self.fullscreen = fullscreen;
        }

        info!(
            "Loaded config: {}x{} render, {}x{} window, fps={}, vsync={}, fullscreen={}",
            self.render_width,
            self.render_height,
            self.window_width,
            self.window_height,
            self.target_fps,
            self.vsync,
            self.fullscreen
        );

        Ok(())
    }

    /// Save configuration to the INI file.
    ///
    /// Creates the file if it doesn't exist.
    #[allow(dead_code)]
    pub fn save_to_file(&self) -> Result<(), String> {
        let mut config = Ini::new();

        // [render] section
        config.set("render", "width", Some(self.render_width.to_string()));
        config.set("render", "height", Some(self.render_height.to_string()));

        // [window] section
        config.set("window", "width", Some(self.window_width.to_string()));
        config.set("window", "height", Some(self.window_height.to_string()));
        config.set("window", "target_fps", Some(self.target_fps.to_string()));
        config.set("window", "vsync", Some(self.vsync.to_string()));
        config.set("window", "fullscreen", Some(self.fullscreen.to_string()));

        config
            .write(&self.config_path)
            .map_err(|e| format!("Failed to save config file: {}", e))?;

        info!("Saved config to {:?}", self.config_path);

        Ok(())
    }

    /// Set render resolution.
    #[allow(dead_code)]
    pub fn set_render_size(&mut self, width: u32, height: u32) {
        self.render_width = width;
        self.render_height = height;
    }

    /// Set window size.
    #[allow(dead_code)]
    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.window_width = width;
        self.window_height = height;
    }

    /// Get the window size.
    pub fn window_size(&self) -> (u32, u32) {
        (self.window_width, self.window_height)
    }
}
