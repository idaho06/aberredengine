use raylib::ffi::TraceLogLevel;

use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::gameconfig::GameConfig;
use crate::resources::render::rendertarget::RenderTarget;

impl EngineBuilder {
    pub(super) fn load_config(&self) -> Result<GameConfig, EngineError> {
        let mut config = GameConfig::with_path(&self.config_path);
        if let Some(content) = &self.config_str {
            config
                .load_from_str(content)
                .map_err(|message| EngineError::ConfigEmbedded { message })?;
        } else {
            config
                .load_from_file()
                .map_err(|message| EngineError::ConfigFile {
                    path: self.config_path.clone(),
                    message,
                })?;
        }
        if let Some(title) = &self.title_override {
            config.window_title = title.clone();
        }
        Ok(config)
    }

    fn raylib_log_level_from_env() -> TraceLogLevel {
        std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .map(Self::raylib_log_level_from_rust_log)
            .unwrap_or(TraceLogLevel::LOG_INFO)
    }

    pub(super) fn raylib_log_level_from_rust_log(rust_log: &str) -> TraceLogLevel {
        let default_directive = rust_log
            .split(',')
            .map(str::trim)
            .find(|directive| !directive.is_empty() && !directive.contains('='));

        let level = default_directive
            .and_then(|directive| directive.split('/').next())
            .map(|directive| directive.trim().to_ascii_lowercase());

        match level.as_deref() {
            Some("trace") => TraceLogLevel::LOG_TRACE,
            Some("debug") => TraceLogLevel::LOG_DEBUG,
            Some("info") => TraceLogLevel::LOG_INFO,
            Some("warn") | Some("warning") => TraceLogLevel::LOG_WARNING,
            Some("error") => TraceLogLevel::LOG_ERROR,
            Some("off") => TraceLogLevel::LOG_NONE,
            _ => TraceLogLevel::LOG_INFO,
        }
    }

    pub(super) fn setup_window(
        config: &GameConfig,
    ) -> Result<(raylib::RaylibHandle, raylib::RaylibThread, RenderTarget), EngineError> {
        let raylib_log_level = Self::raylib_log_level_from_env();
        let (mut rl, thread) = raylib::init()
            .size(config.window_width as i32, config.window_height as i32)
            .resizable()
            .title(&config.window_title)
            .log_level(raylib_log_level)
            .highdpi()
            .msaa_4x()
            .build();
        rl.set_target_fps(config.target_fps);
        rl.set_exit_key(None);

        let render_target =
            RenderTarget::new(&mut rl, &thread, config.render_width, config.render_height)
                .map_err(|message| EngineError::RenderTarget { message })?;

        Ok((rl, thread, render_target))
    }
}
