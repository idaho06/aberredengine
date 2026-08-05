use super::builder::EngineBuilder;
use aberred_core::error::EngineError;

impl EngineBuilder {
    pub(super) fn validate_builder(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        self.validate_lua_conflicts(use_scene_manager)?;
        self.validate_scene_manager(use_scene_manager)?;
        self.validate_deterministic()?;
        self.validate_replay()?;
        Ok(())
    }

    /// Whether `.with_lua()` was called -- always `false` when the `lua`
    /// feature is disabled (`lua_script` doesn't exist on the builder then).
    fn has_lua_script(&self) -> bool {
        #[cfg(feature = "lua")]
        {
            self.lua_script.is_some()
        }
        #[cfg(not(feature = "lua"))]
        {
            false
        }
    }

    /// Checks `.with_lua()` against `.add_scene()` and against any explicitly
    /// called `.on_*()` hook, regardless of call order (`.with_lua()`
    /// installs its own four hooks unconditionally, so by validation time
    /// the hook `Option` fields alone can't tell "user set this" apart from
    /// "with_lua set this" -- that's what `first_user_hook` tracks separately).
    fn validate_lua_conflicts(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        if !self.has_lua_script() {
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

    /// `.deterministic(seed)` and `.with_lua()` are mutually exclusive --
    /// Lua is outside the deterministic envelope.
    fn validate_deterministic(&self) -> Result<(), EngineError> {
        if self.deterministic_seed.is_some() && self.has_lua_script() {
            return Err(EngineError::LuaConflictsWithDeterministic);
        }
        Ok(())
    }

    /// `.record_replay()`/`.play_replay()` are mutually exclusive, both stay
    /// outside Lua's envelope, recording needs an explicit seed up front, and
    /// playback supplies its own seed from the file. An explicit
    /// `.deterministic()` alongside playback is ambiguous about which seed
    /// wins, so validation rejects it rather than silently preferring one.
    fn validate_replay(&self) -> Result<(), EngineError> {
        if self.record_replay_path.is_some() && self.play_replay_path.is_some() {
            return Err(EngineError::RecordAndPlayReplayConflict);
        }
        if (self.record_replay_path.is_some() || self.play_replay_path.is_some())
            && self.has_lua_script()
        {
            return Err(EngineError::LuaConflictsWithReplay);
        }
        if self.record_replay_path.is_some() && self.deterministic_seed.is_none() {
            return Err(EngineError::RecordReplayRequiresDeterministic);
        }
        if self.play_replay_path.is_some() && self.deterministic_seed.is_some() {
            return Err(EngineError::PlayReplayConflictsWithDeterministic);
        }
        Ok(())
    }
}
