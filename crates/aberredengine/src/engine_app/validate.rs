use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::signal_keys as sk;
use aberred_core::resources::worldsignals::MAX_GROUP_NAME_LEN;

/// Registers and enters [`sk::MAIN_SCENE`] when the game registers no scene of
/// its own: no `.add_scene()`, no `.initial_scene()` and no Lua (which drives
/// scenes from `main.lua`). Shared by `EngineBuilder::try_run` and
/// `TestWorldBuilder::build`.
pub(crate) fn ensure_main_scene(
    scenes: &mut Vec<String>,
    initial_scene: &mut Option<String>,
    has_lua: bool,
) {
    if scenes.is_empty() && initial_scene.is_none() && !has_lua {
        scenes.push(sk::MAIN_SCENE.to_owned());
        *initial_scene = Some(sk::MAIN_SCENE.to_owned());
    }
}

impl EngineBuilder {
    /// See [`ensure_main_scene`]. Runs before validation, so
    /// `.add_scene_system("main", ..)` and friends validate against it.
    pub(super) fn ensure_main_scene(&mut self) {
        let has_lua = self.has_lua_script();
        ensure_main_scene(&mut self.scenes, &mut self.initial_scene, has_lua);
    }

    pub(super) fn validate_builder(&self) -> Result<(), EngineError> {
        self.validate_lua_conflicts()?;
        self.validate_scene_manager()?;
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
    pub(super) fn has_lua_script(&self) -> bool {
        #[cfg(feature = "lua")]
        {
            self.lua_script.is_some()
        }
        #[cfg(not(feature = "lua"))]
        {
            false
        }
    }

    /// Checks `.with_lua()` against `.add_scene()` and `.on_setup()`: a Lua
    /// game's setup is `main.lua`'s `on_setup()`.
    fn validate_lua_conflicts(&self) -> Result<(), EngineError> {
        if !self.has_lua_script() {
            return Ok(());
        }
        if !self.scenes.is_empty() {
            return Err(EngineError::LuaConflictsWithSceneManager);
        }
        if self.setup_hook.is_some() {
            return Err(EngineError::LuaConflictsWithSetup);
        }
        Ok(())
    }

    /// Checks `.add_scene()`/`.initial_scene()` consistency: `initial_scene` is
    /// set and matches a registered scene name, or, with no scenes (Lua),
    /// `initial_scene` is unset.
    fn validate_scene_manager(&self) -> Result<(), EngineError> {
        if self.scenes.is_empty() {
            if self.initial_scene.is_some() {
                return Err(EngineError::InitialSceneWithoutScenes);
            }
            return Ok(());
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
        if self.loading_scene == Some(initial_scene.as_str()) {
            return Err(EngineError::LoadingSceneIsInitialScene {
                name: initial_scene.clone(),
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
