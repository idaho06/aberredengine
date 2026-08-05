//! Refreshes `WindowSize` from the OS each render frame.

use bevy_ecs::prelude::*;

use crate::resources::windowsize::WindowSize;

/// Refreshes `WindowSize` from the OS before input sampling -- first step
/// of the render schedule, since `sample_and_send_input` needs the
/// up-to-date size for its letterbox math.
pub fn refresh_window_size(rl: NonSend<raylib::RaylibHandle>, mut window_size: ResMut<WindowSize>) {
    window_size.w = rl.get_screen_width();
    window_size.h = rl.get_screen_height();
}
