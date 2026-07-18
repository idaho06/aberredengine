use super::builder::EngineBuilder;
use crate::error::EngineError;

impl EngineBuilder {
    pub(super) fn validate_builder(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        self.validate_lua_conflicts(use_scene_manager)?;
        self.validate_scene_manager(use_scene_manager)?;
        Ok(())
    }

    /// Checks `.with_lua()` against `.add_scene()` and against any explicitly
    /// called `.on_*()` hook, regardless of call order (`.with_lua()`
    /// installs its own four hooks unconditionally, so by validation time
    /// the hook `Option` fields alone can't tell "user set this" apart from
    /// "with_lua set this" -- that's what `first_user_hook` tracks separately).
    fn validate_lua_conflicts(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        #[cfg(feature = "lua")]
        let has_lua_script = self.lua_script.is_some();
        #[cfg(not(feature = "lua"))]
        let has_lua_script = false;

        if !has_lua_script {
            return Ok(());
        }
        if use_scene_manager {
            return Err(EngineError::LuaConflictsWithSceneManager);
        }
        if let Some(hook) = self.first_user_hook {
            return Err(EngineError::LuaConflictsWithHooks { hook });
        }
        Ok(())
    }

    /// Checks `.add_scene()`/`.initial_scene()` consistency: no conflicting
    /// hooks, `initial_scene` is set and matches a registered scene name
    /// (SceneManager path), or `initial_scene` is unset (non-SceneManager path).
    fn validate_scene_manager(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        if !use_scene_manager {
            if self.initial_scene.is_some() {
                return Err(EngineError::InitialSceneWithoutScenes);
            }
            return Ok(());
        }

        if self.switch_scene_hook.is_some() {
            return Err(EngineError::AddSceneConflictsWithSwitchScene);
        }
        if self.enter_play_hook.is_some() {
            return Err(EngineError::AddSceneConflictsWithEnterPlay);
        }
        let Some(initial_scene) = &self.initial_scene else {
            return Err(EngineError::AddSceneRequiresInitialScene);
        };
        if !self.scenes.iter().any(|(name, _)| name == initial_scene) {
            let registered = self
                .scenes
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(EngineError::InitialSceneNotRegistered {
                name: initial_scene.clone(),
                registered,
            });
        }
        Ok(())
    }
}
