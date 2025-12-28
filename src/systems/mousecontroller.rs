//! Mouse controller system.
//!
//! Updates entity positions to follow the mouse cursor. Entities with a
//! [`MouseControlled`](crate::components::inputcontrolled::MouseControlled)
//! component will have their [`MapPosition`](crate::components::mapposition::MapPosition)
//! updated based on the mouse's world-space position.

use crate::components::inputcontrolled::MouseControlled;
use crate::components::mapposition::MapPosition;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::screensize::ScreenSize;
use crate::resources::windowsize::WindowSize;
use bevy_ecs::prelude::*;

/// Update each mouse-controlled entity's `MapPosition` position based on mouse's world position.
///
/// The mouse position is transformed from window space → game space → world space
/// to correctly handle letterboxing/pillarboxing when the window is resized.
pub fn mouse_controller(
    mut query: Query<(&MouseControlled, &mut MapPosition)>,
    camera_res: Res<Camera2DRes>,
    window_size: Res<WindowSize>,
    screen_size: Res<ScreenSize>,
    rl: NonSend<raylib::RaylibHandle>,
) {
    // Get mouse position in window coordinates
    let window_mouse_pos = rl.get_mouse_position();

    // Transform from window space to game/render-target space (accounting for letterboxing)
    let game_mouse_pos = window_size.window_to_game_pos(
        window_mouse_pos,
        screen_size.w as u32,
        screen_size.h as u32,
    );

    // Transform from game/screen space to world space using the camera
    let world_position = rl.get_screen_to_world2D(game_mouse_pos, camera_res.0);

    for (mouse_controlled, mut map_position) in query.iter_mut() {
        if mouse_controlled.follow_x {
            map_position.pos.x = world_position.x;
        }
        if mouse_controlled.follow_y {
            map_position.pos.y = world_position.y;
        }
    }
}
