mod components;
mod events;
mod game;
mod resources;
mod systems;

use crate::events::collision::observe_kill_on_collision;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::switchdebug::observe_switch_debug_event;
use crate::resources::audio::{setup_audio, shutdown_audio};
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::input::{InputState, update_input_state};
use crate::resources::screensize::ScreenSize;
use crate::resources::worldtime::WorldTime;
use crate::systems::animation::animation;
use crate::systems::audio::{poll_audio_events, update_bevy_audio_events};
use crate::systems::collision::collision;
use crate::systems::gamestate::check_pending_state;
use crate::systems::input::keyboard_input;
use crate::systems::movement::movement;
use crate::systems::render::render_system;
use crate::systems::time::update_world_time;
use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
//use raylib::prelude::*;

fn main() {
    println!("Hello, world! This is the Aberred Engine!");
    // --------------- Raylib window & assets ---------------
    let (rl, thread) = raylib::init()
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

    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_non_send_resource(rl);
    world.insert_non_send_resource(thread);
    world.spawn(Observer::new(observe_gamestate_change_event));
    world.flush();

    // Set next GameState to Setup
    {
        let mut next_state = world.resource_mut::<NextGameState>();
        next_state.set(GameStates::Setup);
    }
    world.trigger(GameStateChangedEvent {}); // Call inmediatly to enter Setup state

    // Register a global observer for CollisionEvent that despawns both entities.
    world.spawn(Observer::new(observe_kill_on_collision));
    world.spawn(Observer::new(observe_switch_debug_event));
    // Ensure the observer is registered before we run any systems that may trigger events.
    world.flush();

    let mut update = Schedule::default();
    update.add_systems(check_pending_state);
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
    update.add_systems(render_system);

    update
        .initialize(&mut world)
        .expect("Failed to initialize schedule");

    // --------------- Main loop ---------------
    while !world
        .non_send_resource::<raylib::RaylibHandle>()
        .window_should_close()
    {
        let dt = world
            .non_send_resource::<raylib::RaylibHandle>()
            .get_frame_time();
        update_world_time(&mut world, dt);
        // poll input for this frame
        update_input_state(&mut world); // TODO: make it a system

        update.run(&mut world);

        world.clear_trackers(); // Clear changed components for next frame
    }
    shutdown_audio(&mut world);
}
