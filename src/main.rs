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
mod luarc_generator;
mod stub_generator;
mod systems;

use crate::components::persistent::Persistent;
use crate::events::gamestate::GameStateChangedEvent;
use crate::events::gamestate::observe_gamestate_change_event;
use crate::events::switchdebug::switch_debug_observer;
use crate::events::switchfullscreen::switch_fullscreen_observer;
use crate::resources::audio::{setup_audio, shutdown_audio};
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::gamestate::{GameState, GameStates, NextGameState};
use crate::resources::group::TrackedGroups;
use crate::resources::input::InputState;
use crate::resources::lua_runtime::LuaRuntime;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::shaderstore::ShaderStore;
// use crate::resources::rendertarget::RenderFilter;
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
use crate::systems::gameconfig::apply_gameconfig_changes;
use crate::systems::gamestate::{check_pending_state, state_is_playing};
use crate::systems::gridlayout::gridlayout_spawn_system;
use crate::systems::group::update_group_counts_system;
use crate::systems::input::update_input_state;
use crate::systems::inputaccelerationcontroller::input_acceleration_controller;
use crate::systems::inputsimplecontroller::input_simple_controller;
use crate::systems::luaphase::lua_phase_system;
use crate::systems::luatimer::{lua_timer_observer, update_lua_timers};
use crate::systems::menu::menu_selection_observer;
use crate::systems::menu::{menu_controller_observer, menu_despawn, menu_spawn_system};
use crate::systems::mousecontroller::mouse_controller;
use crate::systems::movement::movement;
use crate::systems::particleemitter::particle_emitter_system;
use crate::systems::render::render_system;
use crate::systems::signalbinding::update_world_signals_binding_system;
use crate::systems::stuckto::stuck_to_entity_system;
use crate::systems::time::update_world_time;
use crate::systems::ttl::ttl_system;
use crate::systems::tween::tween_mapposition_system;
use crate::systems::tween::tween_rotation_system;
use crate::systems::tween::tween_scale_system;
use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;
use clap::Parser;
use std::path::PathBuf;
//use raylib::collision;
//use raylib::prelude::*;

/// Aberred Engine 2D
#[derive(Parser)]
#[command(version, author = "Idaho06 from AkinoSoft! cesar.idaho@gmail.com",
          about = "This is the Aberred Engine 2D! https://github.com/idaho06/aberredengine/")]
struct Cli {
    /// Generate Lua LSP stubs from engine metadata and exit.
    /// Optionally provide a path (default: assets/scripts/engine.lua).
    #[arg(long, value_name = "PATH")]
    create_lua_stubs: Option<Option<PathBuf>>,

    /// Generate .luarc.json for Lua Language Server and exit.
    /// Optionally provide a path (default: assets/scripts/.luarc.json).
    #[arg(long, value_name = "PATH")]
    create_luarc: Option<Option<PathBuf>>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    // Early-exit: generate Lua stubs and quit (no window/audio needed)
    if let Some(maybe_path) = cli.create_lua_stubs {
        let path = maybe_path.unwrap_or_else(|| PathBuf::from("assets/scripts/engine.lua"));
        let runtime =
            LuaRuntime::new().expect("Failed to create Lua runtime for stub generation");
        match stub_generator::generate_stubs(&runtime) {
            Ok(content) => {
                if let Err(e) = stub_generator::write_stubs(&path, &content) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                println!("Lua stubs written to {}", path.display());
            }
            Err(e) => {
                eprintln!("Error generating stubs: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Early-exit: generate .luarc.json and quit (no window/audio needed)
    if let Some(maybe_path) = cli.create_luarc {
        let path = maybe_path.unwrap_or_else(|| PathBuf::from("assets/scripts/.luarc.json"));
        let runtime =
            LuaRuntime::new().expect("Failed to create Lua runtime for .luarc.json generation");
        match luarc_generator::generate_luarc(&runtime, "engine.lua") {
            Ok(content) => {
                if let Err(e) = luarc_generator::write_luarc(&path, &content) {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
                println!(".luarc.json written to {}", path.display());
            }
            Err(e) => {
                eprintln!("Error generating .luarc.json: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    log::info!("Hello, world! This is the Aberred Engine!");
    // --------------- Raylib window & assets ---------------
    // GameConfig - will load from config.ini on first frame via apply_gameconfig_changes
    let mut config = GameConfig::new();
    config.load_from_file().ok(); // ignore errors, use defaults

    let window_width = config.window_width;
    let window_height = config.window_height;

    let (mut rl, thread) = raylib::init()
        .size(window_width as i32, window_height as i32)
        .resizable()
        .title("Aberred Engine")
        .build();
    rl.set_target_fps(120);
    // Disable ESC to exit
    rl.set_exit_key(None);

    // --------------- Render target for fixed-resolution rendering ---------------
    let render_width = config.render_width;
    let render_height = config.render_height;

    let render_target = RenderTarget::new(&mut rl, &thread, render_width, render_height)
        .expect("Failed to create render target");
    //render_target.set_filter(RenderFilter::Nearest);
    // --------------- ECS world + resources ---------------
    let mut world = World::new();
    world.insert_resource(WorldTime::default().with_time_scale(1.0));
    world.insert_resource(WorldSignals::default());
    world.insert_resource(TrackedGroups::default());
    // ScreenSize is the game's internal render resolution (updated by apply_gameconfig_changes)
    world.insert_resource(ScreenSize {
        w: render_width as i32,
        h: render_height as i32,
    });
    // WindowSize is the actual window dimensions (updated each frame)
    world.insert_resource(WindowSize {
        w: rl.get_screen_width(),
        h: rl.get_screen_height(),
    });

    world.insert_resource(config);
    world.insert_resource(InputState::default());
    world.insert_non_send_resource(render_target);

    // Init audio
    setup_audio(&mut world); // sets up AudioBridge and Events<AudioEvent> as resources
    // it must go before the game setup!!

    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_non_send_resource(FontStore::new());
    world.insert_non_send_resource(ShaderStore::new());
    world.insert_resource(PostProcessShader::new());

    // Initialize Lua runtime and load main script
    let lua_runtime = LuaRuntime::new().expect("Failed to create Lua runtime");
    if let Err(e) = lua_runtime.run_script("./assets/scripts/main.lua") {
        log::error!("Failed to load main.lua: {}", e);
    }
    world.insert_non_send_resource(lua_runtime);

    world.insert_non_send_resource(rl);
    world.insert_non_send_resource(thread);
    world.spawn((Observer::new(observe_gamestate_change_event), Persistent));

    // Game state systems store
    // NOTE: In bevy_ecs 0.18, registered systems are stored as entities.
    // We must mark them as Persistent so they survive scene transitions.
    let mut systems_store = SystemsStore::new();

    let setup_system_id = world.register_system(game::setup);
    world
        .entity_mut(setup_system_id.entity())
        .insert(Persistent);
    systems_store.insert("setup", setup_system_id);

    let enter_play_system_id = world.register_system(game::enter_play);
    world
        .entity_mut(enter_play_system_id.entity())
        .insert(Persistent);
    systems_store.insert("enter_play", enter_play_system_id);

    let quit_game_system_id = world.register_system(game::quit_game);
    world
        .entity_mut(quit_game_system_id.entity())
        .insert(Persistent);
    systems_store.insert("quit_game", quit_game_system_id);

    let clean_all_entities_system_id = world.register_system(game::clean_all_entities);
    world
        .entity_mut(clean_all_entities_system_id.entity())
        .insert(Persistent);
    systems_store.insert("clean_all_entities", clean_all_entities_system_id);

    let switch_scene_system_id = world.register_system(game::switch_scene);
    world
        .entity_mut(switch_scene_system_id.entity())
        .insert(Persistent);
    systems_store.insert("switch_scene", switch_scene_system_id);

    let menu_despawn_system_id = world.register_system(menu_despawn);
    world
        .entity_mut(menu_despawn_system_id.entity())
        .insert(Persistent);
    systems_store.insert_entity_system("menu_despawn", menu_despawn_system_id);

    world.insert_resource(systems_store);

    world.flush();

    // Set next GameState to Setup
    {
        let mut next_state = world.resource_mut::<NextGameState>();
        next_state.set(GameStates::Setup);
    }
    world.trigger(GameStateChangedEvent {}); // Call inmediatly to enter Setup state

    world.spawn((Observer::new(collision_observer), Persistent));
    world.spawn((Observer::new(switch_debug_observer), Persistent));
    world.spawn((Observer::new(switch_fullscreen_observer), Persistent));
    world.spawn((Observer::new(menu_controller_observer), Persistent));
    world.spawn((Observer::new(menu_selection_observer), Persistent));
    world.spawn((Observer::new(lua_timer_observer), Persistent));
    // Ensure the observer is registered before we run any systems that may trigger events.
    world.flush();

    let mut update = Schedule::default();
    update.add_systems(apply_gameconfig_changes.run_if(state_is_playing)); // Must run early to apply config before other systems
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
    update.add_systems(particle_emitter_system.before(movement)); // Before movement so particles move on spawn frame
    update.add_systems(movement);
    update.add_systems(ttl_system.after(movement));
    update.add_systems(collision_detector.after(mouse_controller).after(movement));
    // Run lua_phase_system AFTER collision detection so phase transitions from collision callbacks
    // are processed in the same frame (before animation_controller evaluates signals)
    update.add_systems(lua_phase_system.after(collision_detector));
    update.add_systems(animation_controller.after(lua_phase_system));
    update.add_systems(animation.after(animation_controller));
    update.add_systems(update_lua_timers);
    update.add_systems(update_world_signals_binding_system);
    update.add_systems(dynamictext_size_system.after(update_world_signals_binding_system));
    update.add_systems(
        (game::update)
            .run_if(state_is_playing)
            .after(check_pending_state)
            .after(lua_phase_system),
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
        let dt = world
            .non_send_resource::<raylib::RaylibHandle>()
            .get_frame_time();
        update_world_time(&mut world, dt);

        update.run(&mut world);

        world.clear_trackers(); // Clear changed components for next frame

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
    }
    shutdown_audio(&mut world);
}
