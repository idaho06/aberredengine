use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::gameconfig::GameConfig;
use log::warn;

impl EngineBuilder {
    /// Builds the startup [`GameConfig`]: the inline string when set, else the
    /// file at the config path. A missing file yields the defaults (with a
    /// warning); an unreadable or unparsable one is an error.
    pub(super) fn load_config(&self) -> Result<GameConfig, EngineError> {
        let mut config = GameConfig::with_path(&self.config_path);
        if let Some(content) = &self.config_str {
            config
                .load_from_str(content)
                .map_err(|message| EngineError::ConfigEmbedded { message })?;
        } else {
            let loaded = config
                .load_from_file()
                .map_err(|message| EngineError::ConfigFile {
                    path: self.config_path.clone(),
                    message,
                })?;
            if !loaded {
                warn!(
                    "Config file {:?} not found; using defaults",
                    self.config_path
                );
            }
        }
        if let Some(title) = &self.title_override {
            config.window_title = title.clone();
        }
        Ok(config)
    }
}
