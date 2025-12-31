//! Aberred Engine main entry point.
//!
//! A 2D game engine written in Rust using:
//! - **raylib** for windowing, graphics, and audio
//! - **bevy_ecs** for entity-component-system architecture
//! - **mlua + LuaJIT** for game logic scripting
//!
//! This executable demonstrates an Arkanoid-style breakout game where most game
//! logic is defined in Lua scripts under `assets/scripts/`.
//!
//! # Project Structure
//!
//! - [`components`] – ECS components (sprites, physics, collision, animation, etc.)
//! - [`events`] – Event types (collision, menu, phase transitions, etc.)
//! - [`game`] – High-level game setup and scene management
//! - [`resources`] – ECS resources (world signals, asset stores, camera, etc.)
//! - [`systems`] – ECS systems (rendering, physics, input, collision, etc.)
//!
//! # Main Loop
//!
//! 1. Initialize raylib window, ECS world, resources (fonts, audio, Lua runtime)
//! 2. Load `main.lua` which calls setup callbacks to load assets
//! 3. Register observers and systems
//! 4. Run the main game loop:
//!    - Update input, timers, physics, collision, animation
//!    - Lua phase callbacks drive game logic
//!    - Render world with camera transforms
//! 5. Clean up audio thread on exit
//!
//! # Running
//!
//! ```sh
//! cargo run --release
//! ```

// Do not create console on Windows
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod components;
mod events;
mod game;
mod resources;
mod systems;

use crate::components::persistent::Persistent;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::switchdebug::switch_debug_observer;
use crate::events::switchfullscreen::switch_fullscreen_observer;
use crate::resources::audio::{setup_audio, shutdown_audio};
use crate::resources::fontstore::FontStore;
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::rendertarget::RenderFilter;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::screensize::ScreenSize;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::animation::animation;
use crate::systems::animation::animation_controller;
use crate::systems::audio::{
    forward_audio_cmds, poll_audio_messages, update_bevy_audio_cmds, update_bevy_audio_messages,
};
use crate::systems::collision::collision_detector;
use crate::systems::collision::collision_observer;
use crate::systems::dynamictext_size::dynamictext_size_system;
use crate::systems::gamestate::{check_pending_state, state_is_playing};
use crate::systems::gridlayout::gridlayout_spawn_system;
use crate::systems::group::update_group_counts_system;
use crate::systems::input::update_input_state;
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
use crate::systems::luaphase::lua_phase_system;
use crate::systems::luatimer::{lua_timer_observer, update_lua_timers};
use crate::systems::menu::menu_selection_observer;
use crate::systems::menu::{menu_controller_observer, menu_spawn_system};
use crate::systems::mousecontroller::mouse_controller;
use crate::systems::movement::movement;
use crate::systems::phase::{phase_change_detector, phase_update_system};
use crate::systems::render::render_system;
use crate::systems::signalbinding::update_world_signals_binding_system;
use crate::systems::stuckto::stuck_to_entity_system;
use crate::systems::time::update_timers;
use crate::systems::time::update_world_time;
use crate::systems::tween::tween_mapposition_system;
use crate::systems::tween::tween_rotation_system;
use crate::systems::tween::tween_scale_system;
use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
//use raylib::collision;
//use raylib::prelude::*;

// Game resolution (fixed internal render size)
const GAME_WIDTH: u32 = 480;
const GAME_HEIGHT: u32 = 270;

fn main() {
    println!("Hello, world! This is the Aberred Engine!");
    // --------------- Raylib window & assets ---------------
    let (mut rl, thread) = raylib::init()
        .size((GAME_WIDTH * 3u32) as i32, (GAME_HEIGHT * 3u32) as i32)
        .resizable()
        .title("Aberred Engine - Arkanoid")
        .vsync()
        .build();
    rl.set_target_fps(120);
    // Disable ESC to exit
    rl.set_exit_key(None);

    // --------------- Render target for fixed-resolution rendering ---------------
    let render_target = RenderTarget::new(&mut rl, &thread, GAME_WIDTH, GAME_HEIGHT)
        .expect("Failed to create render target");
    //render_target.set_filter(RenderFilter::Nearest);
    // --------------- ECS world + resources ---------------
    let mut world = World::new();
    world.insert_resource(WorldTime::default().with_time_scale(1.0));
    world.insert_resource(WorldSignals::default());
    world.insert_resource(TrackedGroups::default());
    // ScreenSize is the game's internal render resolution (fixed)
    world.insert_resource(ScreenSize {
        w: GAME_WIDTH as i32,
        h: GAME_HEIGHT as i32,
    });
    // WindowSize is the actual window dimensions (updated each frame)
    world.insert_resource(WindowSize {
        w: rl.get_screen_width(),
        h: rl.get_screen_height(),
    });
    world.insert_resource(InputState::default());
    world.insert_non_send_resource(render_target);

    // Init audio
    setup_audio(&mut world); // sets up AudioBridge and Events<AudioEvent> as resources
    // it must go before the game setup!!

    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_non_send_resource(FontStore::new());

    // Initialize Lua runtime and load main script
    let lua_runtime = LuaRuntime::new().expect("Failed to create Lua runtime");
    if let Err(e) = lua_runtime.run_script("./assets/scripts/main.lua") {
        eprintln!("Failed to load main.lua: {}", e);
    }
    world.insert_non_send_resource(lua_runtime);

    world.insert_non_send_resource(rl);
    world.insert_non_send_resource(thread);
    world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

    // Game state systems store
    let mut systems_store = SystemsStore::new();

    let setup_system_id = world.register_system(game::setup);
    systems_store.insert("setup", setup_system_id);

    let enter_play_system_id = world.register_system(game::enter_play);
    systems_store.insert("enter_play", enter_play_system_id);

    let quit_game_system_id = world.register_system(game::quit_game);
    systems_store.insert("quit_game", quit_game_system_id);

    let clean_all_entities_system_id = world.register_system(game::clean_all_entities);
    systems_store.insert("clean_all_entities", clean_all_entities_system_id);

    let switch_scene_system_id = world.register_system(game::switch_scene);
    systems_store.insert("switch_scene", switch_scene_system_id);

    world.insert_resource(systems_store);

    world.flush();

    // Set next GameState to Setup
    {
        let mut next_state = world.resource_mut::<NextGameState>();
        next_state.set(GameStates::Setup);
    }
    world.trigger(GameStateChangedEvent {}); // Call inmediatly to enter Setup state

    world.add_observer(collision_observer);
    world.add_observer(switch_debug_observer);
    world.add_observer(switch_fullscreen_observer);
    world.add_observer(menu_controller_observer);
    world.add_observer(menu_selection_observer);
    world.add_observer(lua_timer_observer);
    // Ensure the observer is registered before we run any systems that may trigger events.
    world.flush();

    let mut update = Schedule::default();
    update.add_systems(phase_change_detector);
    update.add_systems(phase_update_system.after(phase_change_detector));
    update.add_systems(menu_spawn_system);
    update.add_systems(gridlayout_spawn_system);
    update.add_systems(update_input_state);
    update.add_systems(check_pending_state);
    update.add_systems(update_group_counts_system);
    update.add_systems(
        // audio systems must be together
        (
            // First, advance AudioCmd messages and forward them to the audio thread
            update_bevy_audio_cmds,
            forward_audio_cmds,
            // Then, pull audio thread messages and advance them
            poll_audio_messages,
            update_bevy_audio_messages,
            // on_audio_event,
        )
            .chain(),
    );
    //update.add_systems(check_input.after(update_input_state)); // is `after` necessary?
    update.add_systems(input_simple_controller);
    update.add_systems(input_acceleration_controller);
    update.add_systems(mouse_controller);
    update.add_systems(stuck_to_entity_system.after(collision_detector));
    update.add_systems(tween_mapposition_system);
    update.add_systems(tween_rotation_system);
    update.add_systems(tween_scale_system);
    update.add_systems(movement);
    update.add_systems(collision_detector.after(mouse_controller).after(movement));
    // Run lua_phase_system AFTER collision detection so phase transitions from collision callbacks
    // are processed in the same frame (before animation_controller evaluates signals)
    update.add_systems(lua_phase_system.after(collision_detector));
    update.add_systems(animation_controller.after(lua_phase_system));
    update.add_systems(animation.after(animation_controller));
    update.add_systems(update_timers);
    update.add_systems(update_lua_timers);
    update.add_systems(update_world_signals_binding_system);
    update.add_systems(dynamictext_size_system.after(update_world_signals_binding_system));
    update.add_systems(
        (game::update)
            .run_if(state_is_playing)
            .after(check_pending_state),
    );
    update.add_systems(render_system.after(collision_detector));

    update
        .initialize(&mut world)
        .expect("Failed to initialize schedule");

    // --------------- Main loop ---------------
    while !world
        .non_send_resource::<raylib::RaylibHandle>()
        .window_should_close()
        && !world.resource::<WorldSignals>().has_flag("quit_game")
    {
        // Update window size each frame (may change due to resize)
        let (new_w, new_h) = {
            let rl = world.non_send_resource::<raylib::RaylibHandle>();
            (rl.get_screen_width(), rl.get_screen_height())
        };
        {
            let mut window_size = world.resource_mut::<WindowSize>();
            window_size.w = new_w;
            window_size.h = new_h;
        }

        let dt = world
            .non_send_resource::<raylib::RaylibHandle>()
            .get_frame_time();
        update_world_time(&mut world, dt);

        update.run(&mut world);

        world.clear_trackers(); // Clear changed components for next frame
    }
    shutdown_audio(&mut world);
}
