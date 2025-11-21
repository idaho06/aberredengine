use crate::components::inputcontrolled::MouseControlled;
use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;
//use crate::components::rigidbody::RigidBody;
use crate::components::mapposition::MapPosition;
use crate::resources::camera2d::Camera2DRes;

/// Update each mouse-controlled entity's `MapPosition` position based on mouses'world position.
pub fn mouse_controller(
    mut query: Query<(&MouseControlled, &mut MapPosition)>,
    camera_res: Res<Camera2DRes>,
    rl: NonSend<raylib::RaylibHandle>,
) {
    let mouse_position = rl.get_mouse_position();
    let world_position = rl.get_screen_to_world2D(mouse_position, camera_res.0);
    for (mouse_controlled, mut map_position) in query.iter_mut() {
        if mouse_controlled.follow_x {
            //eprintln!("Position before: {:?}", map_position.pos);
            map_position.pos.x = world_position.x;
            //eprintln!("Position after: {:?}", map_position.pos);
        }
        if mouse_controlled.follow_y {
            //eprintln!("Position before: {:?}", map_position.pos);
            map_position.pos.y = world_position.y;
            //eprintln!("Position after: {:?}", map_position.pos);
        }
    }
}
