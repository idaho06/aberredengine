//! Errors returned from `EngineBuilder`'s construction/teardown path
//! (`try_run` and everything it calls). Runtime systems do not use this
//! type -- they keep their own `warn!`/`error!` discipline.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(
        "EngineBuilder conflict: .add_scene() and .on_switch_scene() cannot be used \
         together. Use .add_scene() for SceneManager-based games, or \
         .on_switch_scene() for full manual control -- not both."
    )]
    AddSceneConflictsWithSwitchScene,

    #[error(
        "EngineBuilder conflict: .add_scene() and .on_enter_play() cannot be used \
         together. SceneManager owns the enter_play hook. Use .on_setup() for \
         asset loading instead."
    )]
    AddSceneConflictsWithEnterPlay,

    #[error(
        "EngineBuilder: .add_scene() requires .initial_scene(\"name\") to specify \
         which scene to enter first."
    )]
    AddSceneRequiresInitialScene,

    #[error(
        "EngineBuilder conflict: .with_lua() and .add_scene() are mutually exclusive. \
         Lua games drive scenes from main.lua's scene registry; SceneManager games use \
         .add_scene() -- not both."
    )]
    LuaConflictsWithSceneManager,

    #[error(
        "EngineBuilder conflict: .with_lua() replaces the setup/enter_play/update/\
         switch_scene hooks; also calling .{hook}() is ambiguous. Remove the explicit \
         hook call."
    )]
    LuaConflictsWithHooks { hook: &'static str },

    #[error(
        "EngineBuilder: .initial_scene(\"{name}\") does not match any registered scene. \
         Registered scenes: {registered}."
    )]
    InitialSceneNotRegistered { name: String, registered: String },

    #[error(
        "EngineBuilder: .initial_scene() was set but no scenes were registered via \
         .add_scene(). Either add a scene or remove .initial_scene()."
    )]
    InitialSceneWithoutScenes,

    #[error("EngineBuilder missing required system registrations: {0}")]
    MissingSystems(String),

    #[error("Failed to parse embedded config: {message}")]
    ConfigEmbedded { message: String },

    #[error("Failed to load config '{path}': {message}")]
    ConfigFile { path: PathBuf, message: String },

    #[error("Failed to create render target: {message}")]
    RenderTarget { message: String },

    #[error("Failed to initialize imgui bridge: {message}")]
    Imgui { message: String },

    #[cfg(feature = "lua")]
    #[error("Failed to create Lua runtime: {0}")]
    Lua(#[from] mlua::Error),

    #[error("Failed to initialize {which} schedule: {source}")]
    ScheduleInit {
        which: &'static str,
        #[source]
        source: bevy_ecs::schedule::ScheduleBuildError,
    },

    #[error("Failed to spawn logic thread: {source}")]
    ThreadSpawn {
        #[source]
        source: std::io::Error,
    },
}
