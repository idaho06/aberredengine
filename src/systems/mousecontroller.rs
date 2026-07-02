//! Mouse controller system.
//!
//! Updates entity positions to follow the mouse cursor. Entities with a
//! [`MouseControlled`](crate::components::inputcontrolled::MouseControlled)
//! component will have their [`MapPosition`](crate::components::mapposition::MapPosition)
//! updated based on the mouse's world-space position.

use crate::components::inputcontrolled::MouseControlled;
use crate::components::mapposition::MapPosition;
use crate::resources::input::InputState;
use bevy_ecs::prelude::*;

/// Update each mouse-controlled entity's `MapPosition` based on the mouse's
/// world position.
///
/// Reads `InputState.mouse_world_x/y` — the window→game→world transformation
/// (letterbox correction + camera projection) already happened when the
/// per-frame input snapshot was applied, so this system no longer touches
/// raylib. Runs on the FIXED schedule: the value is written once per render
/// frame and held constant across substeps, matching raylib's own
/// once-per-real-frame mouse refresh.
pub fn mouse_controller(
    mut query: Query<(&MouseControlled, &mut MapPosition)>,
    input: Res<InputState>,
) {
    for (mouse_controlled, mut map_position) in query.iter_mut() {
        if mouse_controlled.follow_x {
            map_position.pos.x = input.mouse_world_x;
        }
        if mouse_controlled.follow_y {
            map_position.pos.y = input.mouse_world_y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    #[test]
    fn follows_mouse_world_position_per_axis() {
        let mut world = World::new();
        world.insert_resource(InputState {
            mouse_world_x: 320.0,
            mouse_world_y: -48.0,
            ..Default::default()
        });

        let both = world
            .spawn((
                MouseControlled {
                    follow_x: true,
                    follow_y: true,
                },
                MapPosition::new(1.0, 2.0),
            ))
            .id();
        let x_only = world
            .spawn((
                MouseControlled {
                    follow_x: true,
                    follow_y: false,
                },
                MapPosition::new(1.0, 2.0),
            ))
            .id();

        world.run_system_once(mouse_controller).unwrap();

        let pos = world.get::<MapPosition>(both).unwrap();
        assert_eq!(pos.pos.x, 320.0);
        assert_eq!(pos.pos.y, -48.0);
        let pos = world.get::<MapPosition>(x_only).unwrap();
        assert_eq!(pos.pos.x, 320.0);
        assert_eq!(pos.pos.y, 2.0);
    }
}
