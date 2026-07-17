//! Render thread's own tick stats, produced and consumed entirely
//! render-side (unlike `SimStats`/`AudioStats`, nothing here crosses a
//! thread boundary).

use bevy_ecs::prelude::Resource;

use crate::protocol::stats::ThreadStats;

/// Render thread's per-frame timing, measured around the render schedule's
/// `schedule.run(world)` call -- includes the vsync wait (raylib paces the
/// render thread itself, unlike sim/audio's dedicated `Pacer`), so this is
/// whole-frame time, not "work excluding sleep" the way `SimStats`/
/// `AudioStats` are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct RenderStats(pub ThreadStats);
