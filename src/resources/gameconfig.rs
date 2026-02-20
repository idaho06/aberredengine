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
use raylib::prelude::Color;
use std::path::PathBuf;

/// Default safe values for startup
const DEFAULT_RENDER_WIDTH: u32 = 640;
const DEFAULT_RENDER_HEIGHT: u32 = 360;
const DEFAULT_WINDOW_WIDTH: u32 = 1280;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
const DEFAULT_TARGET_FPS: u32 = 120;
const DEFAULT_VSYNC: bool = true;
const DEFAULT_FULLSCREEN: bool = false;
const DEFAULT_BACKGROUND_COLOR: Color = Color::new(80, 80, 80, 255);
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
    /// Background clear color for the render target.
    pub background_color: Color,
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
            background_color: DEFAULT_BACKGROUND_COLOR,
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
        if let Some(bg) = config.get("render", "background_color") {
            let parts: Vec<&str> = bg.split(',').collect();
            if parts.len() == 3
                && let (Ok(r), Ok(g), Ok(b)) = (
                    parts[0].trim().parse::<u8>(),
                    parts[1].trim().parse::<u8>(),
                    parts[2].trim().parse::<u8>(),
                )
            {
                self.background_color = Color::new(r, g, b, 255);
            }
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
        config.set(
            "render",
            "background_color",
            Some(format!(
                "{},{},{}",
                self.background_color.r, self.background_color.g, self.background_color.b
            )),
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_new_defaults() {
        let config = GameConfig::new();
        assert_eq!(config.render_width, 640);
        assert_eq!(config.render_height, 360);
        assert_eq!(config.window_width, 1280);
        assert_eq!(config.window_height, 720);
        assert_eq!(config.target_fps, 120);
        assert!(config.vsync);
        assert!(!config.fullscreen);
    }

    #[test]
    fn test_default_trait() {
        let config = GameConfig::default();
        assert_eq!(config.render_width, 640);
        assert_eq!(config.target_fps, 120);
    }

    #[test]
    fn test_with_path() {
        let config = GameConfig::with_path("/tmp/custom.ini");
        assert_eq!(config.config_path, PathBuf::from("/tmp/custom.ini"));
        // Other fields should be defaults
        assert_eq!(config.render_width, 640);
    }

    #[test]
    fn test_set_render_size() {
        let mut config = GameConfig::new();
        config.set_render_size(320, 240);
        assert_eq!(config.render_width, 320);
        assert_eq!(config.render_height, 240);
    }

    #[test]
    fn test_set_window_size() {
        let mut config = GameConfig::new();
        config.set_window_size(1920, 1080);
        assert_eq!(config.window_width, 1920);
        assert_eq!(config.window_height, 1080);
    }

    #[test]
    fn test_window_size_getter() {
        let mut config = GameConfig::new();
        config.set_window_size(800, 600);
        assert_eq!(config.window_size(), (800, 600));
    }

    #[test]
    fn test_load_from_file() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_load.ini");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            "[render]\nwidth = 800\nheight = 450\n[window]\nwidth = 1600\nheight = 900\ntarget_fps = 60\nvsync = false\nfullscreen = true"
        )
        .unwrap();

        let mut config = GameConfig::with_path(&path);
        config.load_from_file().unwrap();

        assert_eq!(config.render_width, 800);
        assert_eq!(config.render_height, 450);
        assert_eq!(config.window_width, 1600);
        assert_eq!(config.window_height, 900);
        assert_eq!(config.target_fps, 60);
        assert!(!config.vsync);
        assert!(config.fullscreen);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_from_file_missing_values_keep_defaults() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_partial.ini");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "[render]\nwidth = 320").unwrap();

        let mut config = GameConfig::with_path(&path);
        config.load_from_file().unwrap();

        assert_eq!(config.render_width, 320);
        assert_eq!(config.render_height, 360); // default
        assert_eq!(config.window_width, 1280); // default

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_from_file_nonexistent() {
        let config_result = GameConfig::with_path("/tmp/nonexistent_aberred.ini").load_from_file();
        assert!(config_result.is_err());
    }

    #[test]
    fn test_save_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_roundtrip.ini");

        let mut config = GameConfig::with_path(&path);
        config.set_render_size(400, 300);
        config.set_window_size(800, 600);
        config.target_fps = 30;
        config.vsync = false;
        config.fullscreen = true;
        config.save_to_file().unwrap();

        let mut loaded = GameConfig::with_path(&path);
        loaded.load_from_file().unwrap();

        assert_eq!(loaded.render_width, 400);
        assert_eq!(loaded.render_height, 300);
        assert_eq!(loaded.window_width, 800);
        assert_eq!(loaded.window_height, 600);
        assert_eq!(loaded.target_fps, 30);
        assert!(!loaded.vsync);
        assert!(loaded.fullscreen);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_background_color_save_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_bg_roundtrip.ini");

        let mut config = GameConfig::with_path(&path);
        config.background_color = Color::new(10, 200, 55, 255);
        config.save_to_file().unwrap();

        let mut loaded = GameConfig::with_path(&path);
        loaded.load_from_file().unwrap();

        assert_eq!(loaded.background_color.r, 10);
        assert_eq!(loaded.background_color.g, 200);
        assert_eq!(loaded.background_color.b, 55);
        assert_eq!(loaded.background_color.a, 255);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_background_color_missing_keeps_default() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_bg_missing.ini");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "[render]\nwidth = 320").unwrap();

        let mut config = GameConfig::with_path(&path);
        config.load_from_file().unwrap();

        assert_eq!(config.background_color.r, 80);
        assert_eq!(config.background_color.g, 80);
        assert_eq!(config.background_color.b, 80);

        std::fs::remove_file(&path).ok();
    }
}
