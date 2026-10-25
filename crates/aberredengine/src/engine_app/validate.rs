use super::builder::EngineBuilder;
use aberred_core::error::EngineError;
use aberred_core::resources::signal_keys as sk;
use aberred_core::resources::world_signals::MAX_GROUP_NAME_LEN;

/// Registers [`sk::MAIN_SCENE`] as the initial scene when the game registers
/// no scene of its own (no `.add_scene()`, no `.initial_scene()`), Lua games
/// included. Shared by `EngineBuilder::try_run` and `TestWorldBuilder::build`.
pub(crate) fn ensure_main_scene(scenes: &mut Vec<String>, initial_scene: &mut Option<String>) {
    if scenes.is_empty() && initial_scene.is_none() {
        scenes.push(sk::MAIN_SCENE.to_owned());
        *initial_scene = Some(sk::MAIN_SCENE.to_owned());
    }
}

/// Records that `method` names `scene` in `scene_refs`, for the check that
/// every named scene is registered ([`validate_scene_refs`]), and returns the
/// owned name. Shared by `EngineBuilder` and `TestWorldBuilder`.
pub(crate) fn record_scene_ref(
    scene_refs: &mut Vec<(&'static str, String)>,
    method: &'static str,
    scene: impl Into<String>,
) -> String {
    let scene = scene.into();
    scene_refs.push((method, scene.clone()));
    scene
}

/// Checks `.add_scene()`/`.initial_scene()` consistency: `initial_scene` is
/// set and matches a registered scene name. Runs after [`ensure_main_scene`],
/// so no scenes means `.initial_scene()` was set without `.add_scene()`.
/// Shared by `EngineBuilder::try_run` and `TestWorldBuilder::build`.
pub(crate) fn validate_scene_manager(
    scenes: &[String],
    initial_scene: &Option<String>,
    loading_scene: &Option<String>,
) -> Result<(), EngineError> {
    if scenes.is_empty() {
        return Err(EngineError::InitialSceneWithoutScenes);
    }

    let Some(initial_scene) = initial_scene else {
        return Err(EngineError::AddSceneRequiresInitialScene);
    };
    if !scenes.contains(initial_scene) {
        return Err(EngineError::InitialSceneNotRegistered {
            name: initial_scene.clone(),
            registered: scenes.join(", "),
        });
    }
    if loading_scene.as_deref() == Some(initial_scene.as_str()) {
        return Err(EngineError::LoadingSceneIsInitialScene {
            name: initial_scene.clone(),
        });
    }
    Ok(())
}

/// Every scene named by `add_scene_system`/`on_scene_enter`/`on_scene_exit`
/// must be registered with `.add_scene()`; otherwise the system would never
/// run, or the observer would have no scene entity to attach to. `scene_refs`
/// holds the `(method, scene)` pairs those methods recorded. Shared by
/// `EngineBuilder::try_run` and `TestWorldBuilder::build`.
pub(crate) fn validate_scene_refs(
    scenes: &[String],
    scene_refs: &[(&'static str, String)],
) -> Result<(), EngineError> {
    let registered = |name: &str| scenes.iter().any(|scene| scene == name);
    if let Some((method, name)) = scene_refs.iter().find(|(_, name)| !registered(name)) {
        return Err(EngineError::SceneNotRegistered {
            method,
            name: name.clone(),
            registered: scenes.join(", "),
        });
    }
    Ok(())
}

/// Rejects `.track_group()` names whose group-count signal key would not fit
/// (see `TrackedGroups::add_group`, which only warns at runtime). Shared by
/// `EngineBuilder::try_run` and `TestWorldBuilder::build`.
pub(crate) fn validate_tracked_groups(tracked_groups: &[String]) -> Result<(), EngineError> {
    if let Some(name) = tracked_groups
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

impl EngineBuilder {
    /// See [`ensure_main_scene`]. Runs before validation, so
    /// `.add_scene_system("main", ..)` and friends validate against it.
    pub(super) fn ensure_main_scene(&mut self) {
        ensure_main_scene(&mut self.scenes, &mut self.initial_scene);
    }

    pub(super) fn validate_builder(&self) -> Result<(), EngineError> {
        validate_scene_manager(&self.scenes, &self.initial_scene, &self.loading_scene)?;
        validate_scene_refs(&self.scenes, &self.scene_refs)?;
        self.validate_deterministic()?;
        self.validate_replay()?;
        validate_tracked_groups(&self.tracked_groups)?;
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
