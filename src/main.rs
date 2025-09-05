mod components;
mod events;
mod game;
mod resources;
mod systems;

use crate::events::collision::observe_kill_on_collision;
use crate::events::switchdebug::observe_switch_debug_event;
use crate::resources::audio::{setup_audio, shutdown_audio};
use crate::resources::camera2d::Camera2DRes;
use crate::resources::input::{InputState, update_input_state};
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;
use crate::systems::animation::animation;
use crate::systems::audio::{poll_audio_events, update_bevy_audio_events};
use crate::systems::collision::collision;
use crate::systems::input::keyboard_input;
use crate::systems::movement::movement;
use crate::systems::render::{render_debug_ui, render_pass};
use crate::systems::time::update_world_time;
use bevy_ecs::observer::Observer;
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

    // Disable ESC to exit
    // rl.set_exit_key(None);

    // --------------- ECS world + resources ---------------
    let mut world = World::new();
    world.insert_resource(WorldTime::default());
    world.insert_resource(ScreenSize {
        w: rl.get_screen_width(),
        h: rl.get_screen_height(),
    });
    world.insert_resource(InputState::default());

    // Init audio
    setup_audio(&mut world); // sets up AudioBridge and Events<AudioEvent> as resources
    // it must go before the game setup!!

    // Load textures, create camera, create resources and spawn some example sprites
    // Also loads musics and starts playback
    game::setup(&mut world, &mut rl, &thread);

    // Register a global observer for CollisionEvent that despawns both entities.
    world.spawn(Observer::new(observe_kill_on_collision));
    world.spawn(Observer::new(observe_switch_debug_event));
    // Ensure the observer is registered before we run any systems that may trigger events.
    world.flush();

    let mut update = Schedule::default();
    update.add_systems(
        (
            update_bevy_audio_events,
            // on_audio_event,
            poll_audio_events,
        )
            .chain(),
    );
    update.add_systems(keyboard_input);
    update.add_systems(movement);
    update.add_systems(collision);
    update.add_systems(animation);

    update
        .initialize(&mut world)
        .expect("Failed to initialize schedule");

    // --------------- Main loop ---------------
    while !rl.window_should_close() {
        // game_music.update_stream();
        // call all the systems except render
        let dt = rl.get_frame_time();
        update_world_time(&mut world, dt);
        // poll input for this frame
        update_input_state(&mut world, &rl);

        update.run(&mut world);

        world.clear_trackers(); // Clear changed components for next frame

        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::GRAY);

        // Draw in world coordinates using Camera2D.
        {
            let mut d2 = d.begin_mode2D(world.resource::<Camera2DRes>().0);
            render_pass(&mut world, &mut d2);
            // d2 dropped here at end of this block -> EndMode2D()
        }

        // You can draw screen-space UI with `d` after this point.
        render_debug_ui(&mut world, &mut d);

        // d dropped here -> EndDrawing()
    }
    shutdown_audio(&mut world);
}
