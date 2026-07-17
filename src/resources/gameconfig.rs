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
//! pixel_snap_camera = true
//!
//! [window]
//! width = 1280
//! height = 720
//! fullscreen = false
//! vsync = true
//! target_fps = 120
//! title = Aberred Engine
//!
//! [simulation]
//! hz = 240
//! ; snapshot_hz = 60   ; optional; defaults to [window] target_fps
//!
//! [audio]
//! hz = 100
//!
//! [input]
//! gamepad_deadzone = 0.15
//! ```

use bevy_ecs::prelude::*;
use configparser::ini::Ini;
use log::{debug, info, warn};
use raylib::prelude::Color;
use std::path::PathBuf;

use crate::resources::texturefilter::TextureFilter;

/// Default safe values for startup
const DEFAULT_RENDER_WIDTH: u32 = 640;
const DEFAULT_RENDER_HEIGHT: u32 = 360;
const DEFAULT_WINDOW_WIDTH: u32 = 1280;
const DEFAULT_WINDOW_HEIGHT: u32 = 720;
const DEFAULT_TARGET_FPS: u32 = 120;
const DEFAULT_VSYNC: bool = true;
const DEFAULT_FULLSCREEN: bool = false;
const DEFAULT_PIXEL_SNAP_CAMERA: bool = true;
const DEFAULT_BACKGROUND_COLOR: Color = Color::new(80, 80, 80, 255);
const DEFAULT_CONFIG_PATH: &str = "./config.ini";
const DEFAULT_WINDOW_TITLE: &str = "Aberred Engine";
/// Default sim tick rate, equivalent to a 1/240s fixed step.
const DEFAULT_SIM_HZ: f64 = 240.0;
/// Default audio tick rate, equivalent to a 10ms pump interval.
const DEFAULT_AUDIO_HZ: f64 = 100.0;
/// Sim/audio tick rate clamp range: below 15Hz gameplay feels broken, above
/// 1000Hz is almost certainly a config typo.
const MIN_TICK_HZ: f64 = 15.0;
const MAX_TICK_HZ: f64 = 1000.0;
/// Default gamepad analog-stick deadzone radius (`[input] gamepad_deadzone`).
const DEFAULT_GAMEPAD_DEADZONE: f32 = 0.15;

/// Game configuration resource.
///
/// Stores render resolution, window settings, and other configurable options.
/// On first insertion into the ECS world, the [`apply_gameconfig_changes`]
/// system will attempt to load values from the configuration file.
///
/// [`apply_gameconfig_changes`]: crate::systems::render::apply_gameconfig_changes
#[derive(Resource, Debug, Clone, PartialEq)]
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
    /// Snap the camera/view rect to integer pixels before rendering.
    ///
    /// Eliminates sprite atlas bleeding caused by sub-pixel sampling during
    /// camera movement. Useful for pixel-art games; disable for games with
    /// smooth rotation/zoom (e.g. an asteroids-style game) where sub-pixel
    /// camera motion looks better.
    pub pixel_snap_camera: bool,
    /// Texture filter applied to the render target when blitting to the window.
    ///
    /// `Nearest` (default) gives sharp pixel-art scaling. `Bilinear` or higher
    /// produces smooth interpolation when the window is larger than the game resolution.
    pub render_target_filter: TextureFilter,
    /// Background clear color for the render target.
    pub background_color: Color,
    /// Window title.
    pub window_title: String,
    /// Path to the configuration file.
    pub config_path: PathBuf,
    /// Game/sim thread tick rate in Hz (`[simulation] hz`, default `240.0`).
    ///
    /// Read once at startup by the logic thread's `Pacer` — a runtime
    /// *change* to this field after startup has no effect, since the thread
    /// loop constructs its `Pacer` before entering its loop.
    pub sim_hz: f64,
    /// Audio thread tick rate in Hz (`[audio] hz`, default `100.0`).
    ///
    /// Read once at startup by the audio thread's `Pacer` — same
    /// startup-only caveat as [`sim_hz`](Self::sim_hz).
    pub audio_hz: f64,
    /// Rate at which the sim thread publishes a [`DrawableSnapshot`] into the
    /// render triple buffer (`[simulation] snapshot_hz`).
    ///
    /// Decimated relative to `sim_hz`: the sim ticks gameplay at `sim_hz` but
    /// only packages+publishes a snapshot at this (usually much lower) rate.
    /// Defaults to `target_fps` (no point publishing faster than the render
    /// thread can display) when `target_fps > 0`, else `60.0`; this default
    /// is re-resolved from the current `target_fps` on every config load that
    /// doesn't set `snapshot_hz` explicitly. Clamped the same as `sim_hz`/
    /// `audio_hz`. Read once at startup by the logic thread's decimation
    /// timer — same startup-only caveat as [`sim_hz`](Self::sim_hz).
    ///
    /// [`DrawableSnapshot`]: crate::resources::drawable_snapshot::DrawableSnapshot
    pub snapshot_hz: f64,
    /// Radial deadzone applied to gamepad analog-stick axes before they
    /// drive digital actions via [`InputBinding::GamepadAxis`]
    /// (`[input] gamepad_deadzone`, default `0.15`, clamped to `[0.0, 1.0]`).
    /// Applied sim-side, not render-side, so the raw protocol stays
    /// deadzone-free for a future per-game calibration UI; the raw
    /// `InputState.gamepad_axes`/Lua `input.analog.pad_*` values are
    /// likewise never deadzoned, only the digital-action resolution is.
    ///
    /// [`InputBinding::GamepadAxis`]: crate::resources::input_bindings::InputBinding::GamepadAxis
    pub gamepad_deadzone: f32,
}

/// A snapshot of [`GameConfig`] as loaded at startup, before any runtime
/// mutation. Inserted once into the logic world (`setup_logic_world`) right
/// after the config file is read, so game/editor code that mutates
/// `GameConfig` at runtime (render size, window title, background color,
/// ...) can still ask "what was this field's loaded-from-file default?"
/// without inventing a bespoke capture-resource per field.
#[derive(Resource, Debug, Clone)]
pub struct GameConfigDefaults(pub GameConfig);

/// Clamp a parsed `[simulation] hz` / `[audio] hz` value to
/// `MIN_TICK_HZ..=MAX_TICK_HZ`, warning (and keeping the clamped value,
/// rather than rejecting the whole config load) when out of range.
fn clamp_tick_hz(hz: f64, field: &str) -> f64 {
    let clamped = hz.clamp(MIN_TICK_HZ, MAX_TICK_HZ);
    if clamped != hz {
        warn!("{field} = {hz} out of range [{MIN_TICK_HZ}, {MAX_TICK_HZ}]; clamped to {clamped}");
    }
    clamped
}

/// Clamp a parsed `[input] gamepad_deadzone` value to `0.0..=1.0`, warning
/// (and keeping the clamped value, same convention as [`clamp_tick_hz`])
/// when out of range.
fn clamp_gamepad_deadzone(deadzone: f32, field: &str) -> f32 {
    let clamped = deadzone.clamp(0.0, 1.0);
    if clamped != deadzone {
        warn!("{field} = {deadzone} out of range [0.0, 1.0]; clamped to {clamped}");
    }
    clamped
}

/// `snapshot_hz`'s implicit default: track `target_fps` (no point
/// publishing snapshots faster than the render thread can display), falling
/// back to a flat 60 if `target_fps` is unset/zero. Shared by `new()` and
/// `apply_ini`'s no-explicit-override path so the two can't drift. Also used
/// by `render_main_loop` (Phase 7j) to seed its `StatsWindow` at the same
/// implicit rate the render thread already falls back to.
pub(crate) fn default_snapshot_hz(target_fps: u32) -> f64 {
    if target_fps > 0 { target_fps as f64 } else { 60.0 }
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
            pixel_snap_camera: DEFAULT_PIXEL_SNAP_CAMERA,
            render_target_filter: TextureFilter::default(),
            background_color: DEFAULT_BACKGROUND_COLOR,
            window_title: DEFAULT_WINDOW_TITLE.to_string(),
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            sim_hz: DEFAULT_SIM_HZ,
            audio_hz: DEFAULT_AUDIO_HZ,
            snapshot_hz: default_snapshot_hz(DEFAULT_TARGET_FPS),
            gamepad_deadzone: DEFAULT_GAMEPAD_DEADZONE,
        }
    }

    /// Create a new configuration with a custom config file path.
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
        self.apply_ini(&config);
        Ok(())
    }

    /// Load configuration from an INI string.
    ///
    /// Missing values retain their current (default) values.
    pub fn load_from_str(&mut self, content: &str) -> Result<(), String> {
        let mut config = Ini::new();
        config
            .read(content.to_owned())
            .map_err(|e| format!("Failed to parse config: {}", e))?;
        self.apply_ini(&config);
        Ok(())
    }

    fn apply_ini(&mut self, config: &Ini) {
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
        if let Some(snap) = config.getbool("render", "pixel_snap_camera").ok().flatten() {
            self.pixel_snap_camera = snap;
        }
        if let Some(filter_str) = config.get("render", "render_target_filter") {
            self.render_target_filter =
                TextureFilter::from_opt_str_or_warn(Some(&filter_str), "render_target_filter");
        }
        if let Some(title) = config.get("window", "title") {
            self.window_title = title;
        }
        if let Some(hz) = config.getfloat("simulation", "hz").ok().flatten() {
            self.sim_hz = clamp_tick_hz(hz, "simulation.hz");
        }
        if let Some(hz) = config.getfloat("audio", "hz").ok().flatten() {
            self.audio_hz = clamp_tick_hz(hz, "audio.hz");
        }
        // snapshot_hz: an explicit key wins; otherwise re-resolve the
        // target_fps-tracking default every load (this field's default isn't
        // flat, unlike sim_hz/audio_hz, so "missing key" and "recompute from
        // the value target_fps just took" are the same branch).
        let snapshot_hz = config
            .getfloat("simulation", "snapshot_hz")
            .ok()
            .flatten()
            .unwrap_or_else(|| default_snapshot_hz(self.target_fps));
        self.snapshot_hz = clamp_tick_hz(snapshot_hz, "simulation.snapshot_hz");
        if let Some(dz) = config.getfloat("input", "gamepad_deadzone").ok().flatten() {
            self.gamepad_deadzone = clamp_gamepad_deadzone(dz as f32, "input.gamepad_deadzone");
        }
        info!(
            "Loaded config: {}x{} render, {}x{} window, fps={}, vsync={}, fullscreen={}, title={}, sim_hz={}, audio_hz={}, snapshot_hz={}",
            self.render_width,
            self.render_height,
            self.window_width,
            self.window_height,
            self.target_fps,
            self.vsync,
            self.fullscreen,
            self.window_title,
            self.sim_hz,
            self.audio_hz,
            self.snapshot_hz
        );
    }

    /// Save configuration to the INI file.
    ///
    /// Creates the file if it doesn't exist.
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
        config.set("window", "title", Some(self.window_title.clone()));

        config
            .write(&self.config_path)
            .map_err(|e| format!("Failed to save config file: {}", e))?;

        debug!("Saved config to {:?}", self.config_path);

        Ok(())
    }

    /// Set render resolution.
    pub fn set_render_size(&mut self, width: u32, height: u32) {
        self.render_width = width;
        self.render_height = height;
    }

    /// Set window size.
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
    fn test_load_from_str() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[render]\nwidth = 800\nheight = 600\n")
            .unwrap();
        assert_eq!(config.render_width, 800);
        assert_eq!(config.render_height, 600);
        // unset values retain defaults
        assert_eq!(config.target_fps, DEFAULT_TARGET_FPS);
    }

    #[test]
    fn test_load_from_str_single_section() {
        let mut config = GameConfig::new();
        assert!(config.load_from_str("[window]\ntarget_fps = 45\n").is_ok());
        assert_eq!(config.target_fps, 45);
        assert_eq!(config.render_width, DEFAULT_RENDER_WIDTH);
    }

    #[test]
    fn test_new_defaults_sim_and_audio_hz() {
        let config = GameConfig::new();
        assert_eq!(config.sim_hz, DEFAULT_SIM_HZ);
        assert_eq!(config.audio_hz, DEFAULT_AUDIO_HZ);
    }

    #[test]
    fn test_load_sim_and_audio_hz_from_str() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[simulation]\nhz = 120\n\n[audio]\nhz = 50\n")
            .unwrap();
        assert_eq!(config.sim_hz, 120.0);
        assert_eq!(config.audio_hz, 50.0);
    }

    #[test]
    fn test_sim_and_audio_hz_missing_keep_defaults() {
        let mut config = GameConfig::new();
        config.load_from_str("[render]\nwidth = 800\n").unwrap();
        assert_eq!(config.sim_hz, DEFAULT_SIM_HZ);
        assert_eq!(config.audio_hz, DEFAULT_AUDIO_HZ);
    }

    #[test]
    fn test_sim_hz_out_of_range_is_clamped() {
        let mut config = GameConfig::new();
        config.load_from_str("[simulation]\nhz = 5\n").unwrap();
        assert_eq!(config.sim_hz, MIN_TICK_HZ);

        let mut config = GameConfig::new();
        config.load_from_str("[simulation]\nhz = 5000\n").unwrap();
        assert_eq!(config.sim_hz, MAX_TICK_HZ);
    }

    #[test]
    fn test_audio_hz_out_of_range_is_clamped() {
        let mut config = GameConfig::new();
        config.load_from_str("[audio]\nhz = 0\n").unwrap();
        assert_eq!(config.audio_hz, MIN_TICK_HZ);

        let mut config = GameConfig::new();
        config.load_from_str("[audio]\nhz = 100000\n").unwrap();
        assert_eq!(config.audio_hz, MAX_TICK_HZ);
    }

    #[test]
    fn test_new_defaults_snapshot_hz_to_target_fps() {
        let config = GameConfig::new();
        assert_eq!(config.snapshot_hz, DEFAULT_TARGET_FPS as f64);
    }

    #[test]
    fn test_snapshot_hz_explicit_override() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[simulation]\nsnapshot_hz = 30\n")
            .unwrap();
        assert_eq!(config.snapshot_hz, 30.0);
    }

    #[test]
    fn test_snapshot_hz_tracks_target_fps_when_unset() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[window]\ntarget_fps = 90\n")
            .unwrap();
        assert_eq!(config.snapshot_hz, 90.0);
    }

    #[test]
    fn test_snapshot_hz_out_of_range_is_clamped() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[simulation]\nsnapshot_hz = 1\n")
            .unwrap();
        assert_eq!(config.snapshot_hz, MIN_TICK_HZ);

        let mut config = GameConfig::new();
        config
            .load_from_str("[simulation]\nsnapshot_hz = 5000\n")
            .unwrap();
        assert_eq!(config.snapshot_hz, MAX_TICK_HZ);
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

    #[test]
    fn test_new_defaults_window_title() {
        let config = GameConfig::new();
        assert_eq!(config.window_title, "Aberred Engine");
    }

    #[test]
    fn test_load_window_title_from_file() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_title.ini");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "[window]\ntitle = My Cool Game").unwrap();

        let mut config = GameConfig::with_path(&path);
        config.load_from_file().unwrap();
        assert_eq!(config.window_title, "My Cool Game");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_window_title_missing_keeps_default() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_title_missing.ini");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "[window]\nwidth = 800").unwrap();

        let mut config = GameConfig::with_path(&path);
        config.load_from_file().unwrap();
        assert_eq!(config.window_title, "Aberred Engine");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_render_target_filter_default_is_nearest() {
        let config = GameConfig::new();
        assert_eq!(config.render_target_filter, TextureFilter::Nearest);
    }

    #[test]
    fn test_render_target_filter_parses_from_ini() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[render]\nrender_target_filter = bilinear\n")
            .unwrap();
        assert_eq!(config.render_target_filter, TextureFilter::Bilinear);
    }

    #[test]
    fn test_render_target_filter_missing_keeps_nearest() {
        let mut config = GameConfig::new();
        config.load_from_str("[render]\nwidth = 320\n").unwrap();
        assert_eq!(config.render_target_filter, TextureFilter::Nearest);
    }

    #[test]
    fn test_new_defaults_gamepad_deadzone() {
        let config = GameConfig::new();
        assert_eq!(config.gamepad_deadzone, DEFAULT_GAMEPAD_DEADZONE);
    }

    #[test]
    fn test_load_gamepad_deadzone_from_str() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[input]\ngamepad_deadzone = 0.3\n")
            .unwrap();
        assert_eq!(config.gamepad_deadzone, 0.3);
    }

    #[test]
    fn test_gamepad_deadzone_missing_keeps_default() {
        let mut config = GameConfig::new();
        config.load_from_str("[render]\nwidth = 800\n").unwrap();
        assert_eq!(config.gamepad_deadzone, DEFAULT_GAMEPAD_DEADZONE);
    }

    #[test]
    fn test_gamepad_deadzone_out_of_range_is_clamped() {
        let mut config = GameConfig::new();
        config
            .load_from_str("[input]\ngamepad_deadzone = -0.5\n")
            .unwrap();
        assert_eq!(config.gamepad_deadzone, 0.0);

        let mut config = GameConfig::new();
        config
            .load_from_str("[input]\ngamepad_deadzone = 1.5\n")
            .unwrap();
        assert_eq!(config.gamepad_deadzone, 1.0);
    }

    #[test]
    fn test_window_title_save_and_reload_roundtrip() {
        let dir = std::env::temp_dir().join("aberred_test_config");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_title_roundtrip.ini");

        let mut config = GameConfig::with_path(&path);
        config.window_title = "Test Title".to_string();
        config.save_to_file().unwrap();

        let mut loaded = GameConfig::with_path(&path);
        loaded.load_from_file().unwrap();
        assert_eq!(loaded.window_title, "Test Title");

        std::fs::remove_file(&path).ok();
    }
}
