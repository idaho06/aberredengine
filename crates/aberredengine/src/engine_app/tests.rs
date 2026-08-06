use std::path::PathBuf;

use bevy_ecs::prelude::*;
use crossbeam_channel::{bounded, unbounded};
use raylib::ffi::TraceLogLevel;
use aberred_core::math::Vec2;

use super::builder::EngineBuilder;
use super::logic_thread::run_sim_tick;
use super::logic_world::register_persistent_system;
use aberred_core::components::mapposition::MapPosition;
use aberred_core::components::persistent::Persistent;
use aberred_core::protocol::raw_input::InputSample;
use aberred_core::protocol::raw_input::RawDeviceSnapshot;
use aberred_core::protocol::render_logic::{LogicMsg, RenderMsg};
use aberred_core::protocol::snapshot::SnapshotPublisher;
use aberred_core::resources::drawable_snapshot::DrawableSnapshot;
use aberred_core::resources::gameconfig::GameConfig;
use aberred_core::resources::input::InputState;
use aberred_core::protocol::raw_input::ImguiCaptureState;
use aberred_core::resources::systemsstore::SystemsStore;

#[cfg(feature = "lua")]
use aberred_core::systems::animation::animation_controller;
#[cfg(feature = "lua")]
use aberred_core::systems::phase::phase_system;
#[cfg(feature = "lua")]
use aberred_core::systems::group::update_group_counts_system;
#[cfg(feature = "lua")]
use aberred_lua::systems::luaphase::lua_phase_system;
#[cfg(feature = "lua")]
use aberred_lua::systems::luatimer::update_lua_timers;

#[test]
fn test_builder_default() {
    let builder = EngineBuilder::new();
    assert_eq!(builder.config_path, PathBuf::from("config.ini"));
    assert!(builder.title_override.is_none());
    assert!(builder.setup_hook.is_none());
    assert!(builder.enter_play_hook.is_none());
    assert!(builder.update_hook.is_none());
    assert!(builder.switch_scene_hook.is_none());
    assert!(builder.scenes.is_empty());
    assert!(builder.initial_scene.is_none());
}

#[test]
fn test_builder_config() {
    let builder = EngineBuilder::new().config("custom.ini");
    assert_eq!(builder.config_path, PathBuf::from("custom.ini"));
}

#[test]
fn test_builder_title() {
    let builder = EngineBuilder::new().title("My Game");
    assert_eq!(builder.title_override, Some("My Game".to_string()));
}

#[test]
fn test_raylib_log_level_from_rust_log_defaults_to_info() {
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log(""),
        TraceLogLevel::LOG_INFO
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("mycrate=debug"),
        TraceLogLevel::LOG_INFO
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("nope"),
        TraceLogLevel::LOG_INFO
    );
}

#[test]
fn test_raylib_log_level_from_rust_log_maps_supported_levels() {
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("trace"),
        TraceLogLevel::LOG_TRACE
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("debug"),
        TraceLogLevel::LOG_DEBUG
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("info"),
        TraceLogLevel::LOG_INFO
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("warning"),
        TraceLogLevel::LOG_WARNING
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("error"),
        TraceLogLevel::LOG_ERROR
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("off"),
        TraceLogLevel::LOG_NONE
    );
}

#[test]
fn test_raylib_log_level_from_rust_log_uses_global_directive_only() {
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("warn,mycrate=debug"),
        TraceLogLevel::LOG_WARNING
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("mycrate=debug,trace"),
        TraceLogLevel::LOG_TRACE
    );
    assert_eq!(
        aberred_render::logging::raylib_log_level_from_rust_log("info/foo,mycrate=debug"),
        TraceLogLevel::LOG_INFO
    );
}

#[test]
fn test_builder_title_override_applied_to_config() {
    let mut config = GameConfig::new();
    assert_eq!(config.window_title, "Aberred Engine");
    // Simulate what run() does
    let title_override = Some("My Custom Title".to_string());
    if let Some(title) = &title_override {
        config.window_title = title.clone();
    }
    assert_eq!(config.window_title, "My Custom Title");
}

#[test]
fn test_builder_config_path_applied_to_gameconfig() {
    let custom_path = PathBuf::from("/tmp/my_game.ini");
    let config = GameConfig::with_path(&custom_path);
    assert_eq!(config.config_path, custom_path);
}

fn dummy_setup() {}
fn dummy_enter_play() {}
fn dummy_update() {}
fn dummy_switch_scene() {}

// --- Input edge-latch (run_sim_tick) ---
//
// The paced loop runs exactly one `sim` tick per `Pacer` wakeup, so an
// edge (just_pressed/just_released) fires exactly once, covered below.

/// Counts how many times `InputState.action_1`/`mouse_left_button` were
/// observed with an edge set, for asserting "fires exactly once".
#[derive(Resource, Default)]
struct EdgeFireCounts {
    action_1_pressed: u32,
    action_1_released: u32,
    mouse_pressed: u32,
}

fn count_edges_system(input: Res<InputState>, mut counts: ResMut<EdgeFireCounts>) {
    if input.action_1.just_pressed {
        counts.action_1_pressed += 1;
    }
    if input.action_1.just_released {
        counts.action_1_released += 1;
    }
    if input.mouse_left_button.just_pressed {
        counts.mouse_pressed += 1;
    }
}

fn build_edge_test_world() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(InputState::default());
    world.insert_resource(EdgeFireCounts::default());
    let mut schedule = Schedule::default();
    schedule.add_systems(count_edges_system);
    schedule.initialize(&mut world).expect("schedule init");
    (world, schedule)
}

#[test]
fn run_sim_tick_fires_edge_exactly_once_and_clears_it() {
    // Covers both action_1 (rebindable digital input) and mouse_left_button
    // (raw, non-rebindable) in one pass -- clear_edges() treats every
    // digital field identically, so exercising two of them together is
    // enough to confirm the mechanism isn't field-specific.
    let (mut world, mut schedule) = build_edge_test_world();
    {
        let mut input = world.resource_mut::<InputState>();
        input.action_1.active = true;
        input.action_1.just_pressed = true;
        input.mouse_left_button.active = true;
        input.mouse_left_button.just_pressed = true;
    }

    run_sim_tick(&mut world, &mut schedule);

    let counts = world.resource::<EdgeFireCounts>();
    assert_eq!(counts.action_1_pressed, 1, "edge must fire exactly once");
    assert_eq!(counts.mouse_pressed, 1, "mouse edge must fire exactly once");

    let input = world.resource::<InputState>();
    assert!(
        input.action_1.active,
        "active/held state must never be cleared"
    );
    assert!(
        input.mouse_left_button.active,
        "mouse active must be untouched"
    );
    assert!(
        !input.action_1.just_pressed,
        "edge must be consumed (cleared) after the tick sees it"
    );
    assert!(!input.mouse_left_button.just_pressed, "mouse edge consumed");
}

#[test]
fn run_sim_tick_delivers_press_and_release_in_same_sample() {
    let (mut world, mut schedule) = build_edge_test_world();
    {
        // A fast tap within one render frame: both edges present at once.
        let mut input = world.resource_mut::<InputState>();
        input.action_1.just_pressed = true;
        input.action_1.just_released = true;
    }

    run_sim_tick(&mut world, &mut schedule);

    let counts = world.resource::<EdgeFireCounts>();
    assert_eq!(
        counts.action_1_pressed, 1,
        "press edge must fire exactly once"
    );
    assert_eq!(
        counts.action_1_released, 1,
        "release edge must fire exactly once"
    );
}

// --- Channel enum round-trip smoke test ---

#[test]
fn logic_and_render_msgs_round_trip_across_a_thread() {
    let (tx_logic, rx_logic) = unbounded::<LogicMsg>();
    let (tx_render, rx_render) = unbounded::<RenderMsg>();

    let echo = std::thread::spawn(move || {
        // Receive until Shutdown, echoing a Quit back.
        loop {
            match rx_logic.recv().expect("sender alive") {
                LogicMsg::Shutdown => break,
                LogicMsg::ScreenSize { w, h } => assert_eq!((w, h), (320, 200)),
                _ => {}
            }
        }
        let _ = tx_render.send(RenderMsg::Quit);
    });

    tx_logic
        .send(LogicMsg::ScreenSize { w: 320, h: 200 })
        .unwrap();
    tx_logic.send(LogicMsg::Shutdown).unwrap();

    echo.join().expect("echo thread should exit cleanly");
    assert!(matches!(rx_render.recv().unwrap(), RenderMsg::Quit));
}

// --- Dedicated bounded input channel round-trip smoke test ---

#[test]
fn input_sample_round_trips_across_a_thread() {
    let (tx_input, rx_input) = bounded::<InputSample>(64);

    let echo = std::thread::spawn(move || {
        let sample = rx_input.recv().expect("sender alive");
        assert_eq!(sample.raw.window_w, 800);
        assert_eq!(sample.raw.window_h, 600);
        assert!(
            sample
                .raw
                .is_key_down(raylib::ffi::KeyboardKey::KEY_SPACE as u32)
        );
    });

    let mut raw = RawDeviceSnapshot {
        window_w: 800,
        window_h: 600,
        ..Default::default()
    };
    raw.set_key(raylib::ffi::KeyboardKey::KEY_SPACE as u32);
    tx_input
        .try_send(InputSample {
            raw,
            capture: ImguiCaptureState::default(),
        })
        .unwrap();

    echo.join().expect("echo thread should exit cleanly");
}

// --- triple_buffer snapshot transport ---

#[test]
fn snapshot_triple_buffer_round_trips_across_a_thread() {
    let (snap_in, mut snap_out) =
        triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();
    let mut publisher = SnapshotPublisher(snap_in);

    let writer = std::thread::spawn(move || {
        for i in 0..8 {
            let mut snapshot = DrawableSnapshot::default();
            snapshot.camera.zoom = i as f32;
            publisher.0.write(snapshot);
        }
    });
    writer.join().expect("writer thread should exit cleanly");

    // Latest-wins: after the writer is done, the next read must observe
    // the LAST published value, not an intermediate one queued up
    // somewhere -- there's no queue to begin with.
    assert!(snap_out.update(), "a publish should be pending");
    assert_eq!(snap_out.output_buffer().camera.zoom, 7.0);

    // A second read with no new publish in between reports no update.
    assert!(!snap_out.update());
}

#[test]
fn snapshot_publish_reuses_buffer_capacity_and_shrinks_correctly() {
    use aberred_core::components::sprite::Sprite;
    use aberred_core::components::zindex::ZIndex;
    use aberred_core::resources::drawable_snapshot::MapSpriteEntry;
    use std::sync::Arc;

    fn sprite_entry(id: u32) -> MapSpriteEntry {
        MapSpriteEntry {
            entity: Entity::from_raw_u32(id).unwrap(),
            sprite: Sprite {
                tex_key: Arc::from(""),
                width: 0.0,
                height: 0.0,
                offset: Vec2::default(),
                origin: Vec2::default(),
                flip_h: false,
                flip_v: false,
            },
            position: MapPosition::new(0.0, 0.0),
            z_index: ZIndex(0.0),
            scale: None,
            rotation: None,
            shader: None,
            tint: None,
            shadow: None,
            global_transform: None,
            velocity: None,
        }
    }

    let (snap_in, mut snap_out) =
        triple_buffer::TripleBuffer::new(&DrawableSnapshot::default()).split();
    let mut publisher = SnapshotPublisher(snap_in);

    // First publish: 3 entries, via clone_into_buffer + input_buffer_mut +
    // publish -- the same path send_drawable_snapshot uses.
    let first = DrawableSnapshot {
        map_sprites: vec![sprite_entry(0), sprite_entry(1), sprite_entry(2)],
        ..Default::default()
    };
    first.clone_into_buffer(publisher.0.input_buffer_mut());
    publisher.0.publish();

    // Second publish: 1 entry. Guards against Vec::clone_from leaving a
    // stale trailing entry on shrink when reusing the buffer's existing
    // (3-entry) capacity instead of allocating fresh.
    let second = DrawableSnapshot {
        map_sprites: vec![sprite_entry(9)],
        ..Default::default()
    };
    second.clone_into_buffer(publisher.0.input_buffer_mut());
    publisher.0.publish();

    assert!(snap_out.update(), "a publish should be pending");
    let published = snap_out.output_buffer();
    assert_eq!(
        published.map_sprites.len(),
        1,
        "second publish must shrink to exactly 1 entry, no stale trailing entries"
    );
    assert_eq!(
        published.map_sprites[0].entity,
        Entity::from_raw_u32(9).unwrap()
    );
}

#[test]
fn test_builder_hooks_set() {
    let builder = EngineBuilder::new()
        .on_setup(dummy_setup)
        .on_enter_play(dummy_enter_play)
        .on_update(dummy_update)
        .on_switch_scene(dummy_switch_scene);
    assert!(builder.setup_hook.is_some());
    assert!(builder.enter_play_hook.is_some());
    assert!(builder.update_hook.is_some());
    assert!(builder.switch_scene_hook.is_some());
}

#[test]
fn test_register_persistent_system() {
    let mut world = World::new();
    let mut store = SystemsStore::new();

    fn test_system() {}

    register_persistent_system(&mut world, &mut store, "test", test_system);

    // System should be registered in the store
    let system_id = store.get("test");
    assert!(system_id.is_some());

    // System entity should be marked Persistent
    let entity = system_id.unwrap().entity();
    assert!(world.entity(entity).contains::<Persistent>());
}

#[cfg(feature = "lua")]
#[test]
fn test_builder_with_lua() {
    let builder = EngineBuilder::new().with_lua("assets/scripts/main.lua");
    assert_eq!(
        builder.lua_script,
        Some(PathBuf::from("assets/scripts/main.lua"))
    );
    assert!(builder.setup_hook.is_some());
    assert!(builder.enter_play_hook.is_some());
    assert!(builder.update_hook.is_some());
    assert!(builder.switch_scene_hook.is_some());
}

#[cfg(feature = "lua")]
#[test]
fn test_build_logic_schedules_without_lua_runtime_omits_lua_only_systems() {
    let mut world = World::new();
    let (sim, _present) =
        EngineBuilder::build_logic_schedules(None, Vec::new(), &mut world, false, false)
            .expect("build_logic_schedules should succeed without Lua runtime");
    let sim_type_ids: Vec<_> = sim
        .systems()
        .expect("build_logic_schedules initializes the sim schedule")
        .map(|(_, system)| system.system_type())
        .collect();
    let phase_system_type = IntoSystem::into_system(phase_system).system_type();
    let animation_controller_type = IntoSystem::into_system(animation_controller).system_type();
    let lua_phase_system_type = IntoSystem::into_system(lua_phase_system).system_type();
    let update_lua_timers_type = IntoSystem::into_system(update_lua_timers).system_type();

    let phase_index = sim_type_ids
        .iter()
        .position(|type_id| *type_id == phase_system_type)
        .expect("phase_system should be present in the sim schedule");
    let animation_controller_index = sim_type_ids
        .iter()
        .position(|type_id| *type_id == animation_controller_type)
        .expect("animation_controller should be present in the sim schedule");

    assert!(
        animation_controller_index > phase_index,
        "animation_controller should still run after phase_system"
    );
    assert!(
        !sim_type_ids.contains(&lua_phase_system_type),
        "lua_phase_system should be absent when has_lua is false"
    );
    assert!(
        !sim_type_ids.contains(&update_lua_timers_type),
        "update_lua_timers should be absent when has_lua is false"
    );
}

#[cfg(feature = "lua")]
#[test]
fn test_build_logic_schedules_with_lua_orders_group_counts_before_lua_phase() {
    let mut world = World::new();
    let builder = EngineBuilder::new().with_lua("assets/scripts/main.lua");
    let (sim, present) = EngineBuilder::build_logic_schedules(
        builder.update_hook,
        Vec::new(),
        &mut world,
        true,
        false,
    )
    .expect("build_logic_schedules should succeed with has_lua=true");

    let sim_type_ids: Vec<_> = sim
        .systems()
        .expect("build_logic_schedules initializes the sim schedule")
        .map(|(_, system)| system.system_type())
        .collect();
    let present_type_ids: Vec<_> = present
        .systems()
        .expect("build_logic_schedules initializes the present schedule")
        .map(|(_, system)| system.system_type())
        .collect();

    let index_of = |type_ids: &[std::any::TypeId], type_id, label| -> usize {
        type_ids
            .iter()
            .position(|t| *t == type_id)
            .unwrap_or_else(|| panic!("{label} should be present"))
    };

    let update_group_counts_index = index_of(
        &sim_type_ids,
        IntoSystem::into_system(update_group_counts_system).system_type(),
        "update_group_counts_system",
    );
    let lua_phase_index = index_of(
        &sim_type_ids,
        IntoSystem::into_system(lua_phase_system).system_type(),
        "lua_phase_system",
    );

    assert!(
        update_group_counts_index < lua_phase_index,
        "update_group_counts_system should run before lua_phase_system (both sim-schedule)"
    );

    // lua_plugin::update runs on the sim (240Hz) schedule, alongside
    // update_group_counts_system/lua_phase_system; `present` contains only
    // build_drawable_snapshot/send_drawable_snapshot
    // (forward_render_asset_cmds lives on sim).
    let lua_update_type = IntoSystem::into_system(aberred_lua::lua_plugin::update).system_type();
    assert!(
        sim_type_ids.contains(&lua_update_type),
        "lua_plugin::update should be present in the sim schedule"
    );
    assert!(
        !present_type_ids.contains(&lua_update_type),
        "lua_plugin::update should not be present in the present schedule"
    );
    assert!(
        sim_type_ids
            .iter()
            .position(|t| *t == lua_update_type)
            .unwrap()
            > lua_phase_index,
        "lua_plugin::update should run after lua_phase_system (ordered last among \
         Lua-touching sim systems each tick)"
    );
}

#[test]
fn test_builder_chaining() {
    let builder = EngineBuilder::new()
        .config("test.ini")
        .title("Test Game")
        .on_setup(dummy_setup)
        .on_enter_play(dummy_enter_play)
        .on_update(dummy_update)
        .on_switch_scene(dummy_switch_scene);

    assert_eq!(builder.config_path, PathBuf::from("test.ini"));
    assert_eq!(builder.title_override, Some("Test Game".to_string()));
    assert!(builder.setup_hook.is_some());
    assert!(builder.enter_play_hook.is_some());
    assert!(builder.update_hook.is_some());
    assert!(builder.switch_scene_hook.is_some());
}

#[test]
fn test_default_trait() {
    let builder = EngineBuilder::default();
    assert_eq!(builder.config_path, PathBuf::from("config.ini"));
    assert!(builder.title_override.is_none());
}

// --- SceneManager builder tests ---

use aberred_core::systems::GameCtx;
use super::scene::SceneDescriptor;

fn dummy_scene_enter(_ctx: &mut GameCtx) {}
fn dummy_scene_update(_ctx: &mut GameCtx, _dt: f32, _input: &InputState) {}

fn make_descriptor() -> SceneDescriptor {
    SceneDescriptor {
        on_enter: dummy_scene_enter,
        on_update: Some(dummy_scene_update),
        on_exit: None,
        gui_callback: None,
        world_draw_callback: None,
    }
}

#[test]
fn test_add_scene_stores_scenes() {
    let builder = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .add_scene("level1", make_descriptor());
    assert_eq!(builder.scenes.len(), 2);
    assert_eq!(builder.scenes[0].0, "menu");
    assert_eq!(builder.scenes[1].0, "level1");
}

#[test]
fn test_initial_scene_stored() {
    let builder = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .initial_scene("menu");
    assert_eq!(builder.initial_scene, Some("menu".to_string()));
}

#[test]
fn test_add_scene_conflicts_with_on_switch_scene() {
    let err = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .initial_scene("menu")
        .on_switch_scene(dummy_switch_scene)
        .try_run()
        .expect_err("conflicting scene/switch_scene hooks should fail preflight");

    assert!(
        err.to_string()
            .contains("EngineBuilder conflict: .add_scene() and .on_switch_scene()")
    );
}

#[test]
fn test_add_scene_conflicts_with_on_enter_play() {
    let err = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .initial_scene("menu")
        .on_enter_play(dummy_enter_play)
        .try_run()
        .expect_err("conflicting scene/enter_play hooks should fail preflight");

    assert!(
        err.to_string()
            .contains("EngineBuilder conflict: .add_scene() and .on_enter_play()")
    );
}

#[test]
fn test_add_scene_requires_initial_scene() {
    let err = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .try_run()
        .expect_err("missing initial_scene should fail preflight");

    assert!(
        err.to_string()
            .contains(".add_scene() requires .initial_scene")
    );
}

#[cfg(feature = "lua")]
#[test]
fn test_with_lua_conflicts_with_add_scene() {
    let err = EngineBuilder::new()
        .with_lua("assets/scripts/main.lua")
        .add_scene("menu", make_descriptor())
        .initial_scene("menu")
        .try_run()
        .expect_err("with_lua + add_scene should fail preflight");

    assert!(
        err.to_string()
            .contains("EngineBuilder conflict: .with_lua() and .add_scene()")
    );
}

#[cfg(feature = "lua")]
#[test]
fn test_with_lua_conflicts_with_user_hook_either_order() {
    let lua_first = EngineBuilder::new()
        .with_lua("assets/scripts/main.lua")
        .on_setup(dummy_setup)
        .try_run()
        .expect_err("with_lua + on_setup should fail preflight regardless of order");
    let hook_first = EngineBuilder::new()
        .on_setup(dummy_setup)
        .with_lua("assets/scripts/main.lua")
        .try_run()
        .expect_err("on_setup + with_lua should fail preflight regardless of order");

    for err in [lua_first, hook_first] {
        assert!(
            err.to_string()
                .contains("EngineBuilder conflict: .with_lua()")
        );
        assert!(err.to_string().contains("on_setup"));
    }
}

#[test]
fn test_initial_scene_not_registered() {
    let err = EngineBuilder::new()
        .add_scene("menu", make_descriptor())
        .initial_scene("menuu")
        .try_run()
        .expect_err("typo'd initial_scene should fail preflight");

    let err_str = err.to_string();
    assert!(err_str.contains("\"menuu\""));
    assert!(err_str.contains("menu"));
}

#[test]
fn test_initial_scene_without_scenes() {
    let err = EngineBuilder::new()
        .initial_scene("menu")
        .try_run()
        .expect_err("initial_scene without add_scene should fail preflight");

    assert!(
        err.to_string()
            .contains(".initial_scene() was set but no scenes were registered")
    );
}

#[test]
fn test_validate_required_systems_reports_missing_entries() {
    let systems_store = SystemsStore::new();
    let err = EngineBuilder::validate_required_systems(&systems_store, true)
        .expect_err("missing required systems should fail validation");
    let err_str = err.to_string();

    assert!(err_str.contains("setup"));
    assert!(err_str.contains("enter_play"));
    assert!(err_str.contains("quit_game"));
    assert!(err_str.contains("switch_scene"));
}
