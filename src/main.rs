mod components;
mod game;
mod resources;
mod systems;

use crate::resources::camera2d::Camera2DRes;
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;
use crate::systems::collision::collision;
use crate::systems::movement::movement;
use crate::systems::render::render_pass;
use crate::systems::time::update_world_time;
use bevy_ecs::prelude::*;
use raylib::prelude::*;

fn main() {
    println!("Hello, world! This is the Aberred Engine!");
    // --------------- Raylib window & assets ---------------
    let (mut rl, thread) = raylib::init()
        .size(800, 450)
        .title("Aberred Engine")
        .vsync()
        .build();

    // --------------- ECS world + resources ---------------
    let mut world = World::new();
    world.insert_resource(WorldTime::default());
    world.insert_resource(ScreenSize {
        w: rl.get_screen_width(),
        h: rl.get_screen_height(),
    });

    // Load textures, create camera, create resources and spawn some example sprites
    game::setup(&mut world, &mut rl, &thread);

    let mut update = Schedule::default();
    update.add_systems(movement);
    update.add_systems(collision);
    update
        .initialize(&mut world)
        .expect("Failed to initialize schedule");

    // --------------- Main loop ---------------
    while !rl.window_should_close() {
        // call all the systems except render
        let dt = rl.get_frame_time();
        update_world_time(&mut world, dt);

        update.run(&mut world);

        world.clear_trackers(); // Clear changed components for next frame

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);

        // Draw in world coordinates using Camera2D.
        let mut d2 = d.begin_mode2D(world.resource::<Camera2DRes>().0);
        render_pass(&mut world, &mut d2);
        // d2 dropped here -> EndMode2D()

        // You can draw screen-space UI with `d` after this point.

        // d dropped here -> EndDrawing()
    }
}
