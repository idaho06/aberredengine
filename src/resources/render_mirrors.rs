//! Render-world mirrors of `DrawableSnapshot`'s "global" fields (Phase 7f-2).
//!
//! `DrawableSnapshot` (`src/resources/drawable_snapshot.rs`) remains the
//! shared wire-format struct built by `build_drawable_snapshot` on the sim
//! side and published through the triple buffer -- it is NOT shrunk by this
//! phase. What changes is the render side: `receive_snapshot`
//! (`src/engine_app.rs`) fans out a copy of each of these 10 fields into its
//! own dedicated resource here, and `render_system` reads those instead of
//! `snapshot.<field>` directly. Each is a thin wrapper (not a bare re-insert
//! of the underlying type) even where the underlying type already derives
//! `Resource` on the sim side -- the wrapper is the signal that a given
//! render-world resource is a read-only snapshot copy, not live sim state,
//! which matters since several of these types (e.g. `GameConfig`,
//! `WorldTime`) are also live, mutable resources in the sim/logic world.
//! All are written exclusively by `receive_snapshot` and read only by
//! render-side systems.

use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use raylib::prelude::Camera2D;

use crate::resources::appstate::AppState;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::drawable_snapshot::DebugSnapshot;
use crate::resources::gameconfig::GameConfig;
use crate::resources::guitheme::GuiThemeStore;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::worldsignals::SignalSnapshot;
use crate::resources::worldtime::WorldTime;

/// Mirrors `DrawableSnapshot.camera`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderCamera(pub Camera2D);

/// Mirrors `DrawableSnapshot.game_config`. Seeded with the real loaded
/// config at startup, not `::default()` -- see `setup_render_world`'s
/// comment on the sibling `DrawableSnapshot` seed for why.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderGameConfig(pub GameConfig);

/// Mirrors `DrawableSnapshot.signals`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderSignalSnapshot(pub Arc<SignalSnapshot>);

/// Mirrors `DrawableSnapshot.app_state`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderAppState(pub AppState);

/// Mirrors `DrawableSnapshot.debug`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderDebugSnapshot(pub Option<DebugSnapshot>);

/// Mirrors `DrawableSnapshot.active_scene`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderActiveScene(pub Option<Arc<str>>);

/// Mirrors `DrawableSnapshot.world_time`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderWorldTime(pub WorldTime);

/// Mirrors `DrawableSnapshot.post_process`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderPostProcess(pub PostProcessShader);

/// Mirrors `DrawableSnapshot.gui_themes`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderGuiThemes(pub GuiThemeStore);

/// Mirrors `DrawableSnapshot.camera_follow`.
#[derive(Resource, Clone, Debug, Default)]
pub struct RenderCameraFollow(pub CameraFollowConfig);
