//! Bundled Raylib handle + thread `SystemParam`, render-thread-only.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

/// Bundled Raylib handle + thread to reduce system parameter count.
#[derive(SystemParam)]
pub struct RaylibAccess<'w> {
    pub rl: NonSendMut<'w, raylib::RaylibHandle>,
    pub th: NonSend<'w, raylib::RaylibThread>,
}
