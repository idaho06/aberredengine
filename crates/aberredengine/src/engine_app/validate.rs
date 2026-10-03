use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::worldsignals::MAX_GROUP_NAME_LEN;

impl EngineBuilder {
    pub(super) fn validate_builder(&self, use_scene_manager: bool) -> Result<(), EngineError> {
        self.validate_lua_conflicts(use_scene_manager)?;
        self.validate_scene_manager(use_scene_manager)?;
        self.validate_scene_refs()?;
        self.validate_deterministic()?;
        self.validate_replay()?;
        self.validate_tracked_groups()?;
        Ok(())
    }

    /// Rejects `.track_group()` names whose group-count signal key would not
    /// fit (see `TrackedGroups::add_group`, which only warns at runtime).
    fn validate_tracked_groups(&self) -> Result<(), EngineError> {
        if let Some(name) = self
            .tracked_groups
            .iter()
            .find(|name| name.len() > MAX_GROUP_NAME_LEN)
        {
            return Err(EngineError::GroupNameTooLong {
                name: name.clone(),
                len: name.len(),
            });
        }
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
        if !self.scenes.contains(initial_scene) {
            return Err(EngineError::InitialSceneNotRegistered {
                name: initial_scene.clone(),
                registered: self.registered_scene_list(),
            });
        }
        Ok(())
    }

    /// Every scene named by `add_scene_system`/`on_scene_enter`/`on_scene_exit`
    /// must be registered with `.add_scene()`; otherwise the system would never
    /// run, or the observer would have no scene entity to attach to.
    fn validate_scene_refs(&self) -> Result<(), EngineError> {
        let registered = |name: &str| self.scenes.iter().any(|scene| scene == name);
        if let Some(&(method, name)) = self.scene_refs.iter().find(|(_, name)| !registered(name)) {
            return Err(EngineError::SceneNotRegistered {
                method,
                name: name.to_owned(),
                registered: self.registered_scene_list(),
            });
        }
        Ok(())
    }

    /// `"menu, level"`: registered scene names in registration order, for errors.
    fn registered_scene_list(&self) -> String {
        self.scenes.join(", ")
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
