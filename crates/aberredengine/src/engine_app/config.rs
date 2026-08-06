use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::gameconfig::GameConfig;

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
}
