//! Integration tests for the `SceneManager` scene-switch path.
//!
//! Validates that `SceneManager`-based games trigger `SceneEntered`/`SceneExited`
//! in order, track the active scene, and tear down the old scene's entities
//! and entity registrations on every switch.

use aberred_render::resources::scene_table::{GuiCallback, GuiCtx, RenderSceneTable, SceneRender};
use aberredengine::core::events::scene::{SceneEntered, SceneExited};
use aberredengine::core::resources::group::TrackedGroups;
use aberredengine::core::resources::input::InputState;
use aberredengine::core::resources::scenemanager::SceneManager;
use aberredengine::core::resources::systemsstore::SystemsStore;
use aberredengine::core::resources::worldsignals::WorldSignals;
use aberredengine::core::resources::worldtime::WorldTime;
use aberredengine::core::systems::scene_dispatch::{
    SceneLogic, scene_enter_play, scene_switch_poll, scene_switch_system, spawn_scene_entities,
};
use bevy_ecs::message::MessageReader;
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;
use bevy_ecs::system::SystemState;

use aberredengine::core::components::persistent::{CleanableEntity, Persistent};
use aberredengine::core::protocol::audio::AudioCmd;
use aberredengine::core::resources::gamestate::{GameState, NextGameState};

use aberredengine::core::testing::insert_game_ctx_resources;

mod common;

/// Scene events in trigger order: `"enter <name>"` / `"exit <name>"`.
#[derive(Resource, Default)]
struct SceneLog(Vec<String>);

impl SceneLog {
    fn take(world: &mut World) -> Vec<String> {
        std::mem::take(&mut world.resource_mut::<SceneLog>().0)
    }
}

/// A scene with no logic callbacks; its behavior comes from observers.
fn no_callbacks() -> SceneLogic {
    SceneLogic {
        on_enter: |_| {},
        on_update: None,
        on_exit: None,
    }
}

/// A world with `scenes` registered, their scene entities spawned, the
/// `switch_scene` system registered, and persistent global observers logging
/// every scene event into [`SceneLog`]. No scene is active yet.
fn scene_world(scenes: &[&str]) -> World {
    let mut world = World::new();
    insert_game_ctx_resources(&mut world);
    world.insert_resource(WorldTime::default().with_time_scale(1.0));
    world.insert_resource(TrackedGroups::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_resource(InputState::default());
    world.init_resource::<SceneLog>();

    let mut scene_manager = SceneManager::new();
    scene_manager.initial_scene = scenes.first().map(|name| name.to_string());
    for name in scenes {
        scene_manager.insert(*name, no_callbacks());
    }
    world.insert_resource(scene_manager);
    spawn_scene_entities(&mut world);

    let switch = world.register_system(scene_switch_system);
    world.entity_mut(switch.entity()).insert(Persistent);
    world
        .resource_mut::<SystemsStore>()
        .insert("switch_scene", switch);

    world.spawn((
        Observer::new(|ev: On<SceneEntered>, mut log: ResMut<SceneLog>| {
            log.0.push(format!("enter {}", ev.name));
        }),
        Persistent,
    ));
    world.spawn((
        Observer::new(|ev: On<SceneExited>, mut log: ResMut<SceneLog>| {
            log.0.push(format!("exit {}", ev.name));
        }),
        Persistent,
    ));
    world
}

/// Enters the initial scene the way the engine does on `Playing`.
fn enter_play(world: &mut World) {
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();
}

/// Switches straight to `target` through `scene_switch_system`.
fn switch_to(world: &mut World, target: &str) {
    world
        .resource_mut::<WorldSignals>()
        .set_string("scene", target);
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();
}

fn active_scene(world: &World) -> Option<&str> {
    world.resource::<SceneManager>().active_scene.as_deref()
}

// ---------------------------------------------------------------------------
// Test 1: SceneEntered fires for the initial scene on enter_play
// ---------------------------------------------------------------------------

#[test]
fn initial_scene_entered_on_enter_play() {
    let mut world = scene_world(&["menu"]);

    enter_play(&mut world);

    assert_eq!(SceneLog::take(&mut world), ["enter menu"]);
    assert_eq!(active_scene(&world), Some("menu"));
}

// ---------------------------------------------------------------------------
// Test 2: SceneExited fires before SceneEntered on a scene switch
// ---------------------------------------------------------------------------

#[test]
fn exit_triggered_before_enter_on_switch() {
    let mut world = scene_world(&["menu", "level1"]);
    enter_play(&mut world);
    SceneLog::take(&mut world);

    switch_to(&mut world, "level1");

    assert_eq!(SceneLog::take(&mut world), ["exit menu", "enter level1"]);
    assert_eq!(active_scene(&world), Some("level1"));
}

// ---------------------------------------------------------------------------
// Test 3: Switching to an unregistered scene name keeps the current scene
// ---------------------------------------------------------------------------

#[test]
fn unknown_scene_name_keeps_the_current_scene() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);
    SceneLog::take(&mut world);

    // Should NOT panic — logs an error
    switch_to(&mut world, "nonexistent");

    // The target is checked first, so "menu" is never exited
    assert!(SceneLog::take(&mut world).is_empty());
    assert_eq!(active_scene(&world), Some("menu"));
}

// ---------------------------------------------------------------------------
// Test 4: Non-persistent entities are despawned on scene switch
// ---------------------------------------------------------------------------

#[test]
fn non_persistent_entities_despawned() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);

    world.spawn(Persistent);
    world.spawn(());
    world.spawn(());

    // Re-enter "menu"
    switch_to(&mut world, "menu");

    let non_persistent: Vec<Entity> = world
        .query_filtered::<Entity, CleanableEntity>()
        .iter(&world)
        .collect();
    assert!(
        non_persistent.is_empty(),
        "Non-persistent entities should be despawned; found {}",
        non_persistent.len()
    );
}

// ---------------------------------------------------------------------------
// Test 5: SceneManager tracks the active scene through multiple switches
// ---------------------------------------------------------------------------

#[test]
fn active_scene_tracked_through_multiple_switches() {
    let mut world = scene_world(&["menu", "level1"]);

    enter_play(&mut world);
    assert_eq!(active_scene(&world), Some("menu"));
    switch_to(&mut world, "level1");
    assert_eq!(active_scene(&world), Some("level1"));
    switch_to(&mut world, "menu");
    assert_eq!(active_scene(&world), Some("menu"));

    assert_eq!(
        SceneLog::take(&mut world),
        [
            "enter menu",
            "exit menu",
            "enter level1",
            "exit level1",
            "enter menu"
        ]
    );
}

// ---------------------------------------------------------------------------
// Test 6: scene_switch_poll triggers the transition when the flag is set
// ---------------------------------------------------------------------------

#[test]
fn scene_switch_poll_triggers_transition() {
    let mut world = scene_world(&["menu", "level1"]);
    enter_play(&mut world);
    SceneLog::take(&mut world);

    // What a scene system does: request the switch
    world.resource_mut::<WorldSignals>().request_scene("level1");
    world.run_system_once(scene_switch_poll).unwrap();
    world.flush();

    assert!(!world.resource::<WorldSignals>().has_flag("switch_scene"));
    assert_eq!(active_scene(&world), Some("level1"));
    assert_eq!(SceneLog::take(&mut world), ["exit menu", "enter level1"]);
}

// ---------------------------------------------------------------------------
// Test 7: scene_switch_poll is a no-op when the flag is absent
// ---------------------------------------------------------------------------

#[test]
fn scene_switch_poll_noop_without_flag() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);
    SceneLog::take(&mut world);

    world.run_system_once(scene_switch_poll).unwrap();
    world.flush();

    assert_eq!(active_scene(&world), Some("menu"));
    assert!(SceneLog::take(&mut world).is_empty());
}

// ---------------------------------------------------------------------------
// Test 8: Non-persistent registered entity is cleared on scene switch
// ---------------------------------------------------------------------------

#[test]
fn non_persistent_entity_registration_cleared_on_scene_switch() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);

    let player = world.spawn(()).id();
    world
        .resource_mut::<WorldSignals>()
        .set_entity("player", player);

    switch_to(&mut world, "menu");

    assert!(
        world
            .resource::<WorldSignals>()
            .get_entity("player")
            .is_none(),
        "Non-persistent entity registration should be cleared on scene switch"
    );
}

// ---------------------------------------------------------------------------
// Test 9: Persistent registered entity survives scene switch
// ---------------------------------------------------------------------------

#[test]
fn persistent_entity_registration_survives_scene_switch() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);

    let cursor = world.spawn(Persistent).id();
    world
        .resource_mut::<WorldSignals>()
        .set_entity("cursor", cursor);

    switch_to(&mut world, "menu");

    assert_eq!(
        world.resource::<WorldSignals>().get_entity("cursor"),
        Some(cursor),
        "Persistent entity registration should survive scene switch"
    );
}

// ---------------------------------------------------------------------------
// Test 10: Mixed registrations — only non-persistent entries are cleared
// ---------------------------------------------------------------------------

#[test]
fn mixed_registrations_only_non_persistent_cleared_on_scene_switch() {
    let mut world = scene_world(&["menu"]);
    enter_play(&mut world);

    let cursor = world.spawn(Persistent).id();
    let player = world.spawn(()).id();
    let enemy = world.spawn(()).id();
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_entity("cursor", cursor);
        ws.set_entity("player", player);
        ws.set_entity("enemy", enemy);
    }

    switch_to(&mut world, "menu");

    let ws = world.resource::<WorldSignals>();
    assert_eq!(
        ws.get_entity("cursor"),
        Some(cursor),
        "Persistent registration should survive"
    );
    assert!(
        ws.get_entity("player").is_none(),
        "Non-persistent 'player' registration should be cleared"
    );
    assert!(
        ws.get_entity("enemy").is_none(),
        "Non-persistent 'enemy' registration should be cleared"
    );
}

// ---------------------------------------------------------------------------
// Test 11: Scene switching does not emit automatic StopAllMusic
// ---------------------------------------------------------------------------

#[test]
fn scene_switch_does_not_emit_stop_all_music() {
    let mut world = scene_world(&["menu", "level1"]);
    enter_play(&mut world);

    switch_to(&mut world, "level1");

    world.resource_mut::<Messages<AudioCmd>>().update();
    let mut reader_state = SystemState::<MessageReader<AudioCmd>>::new(&mut world);
    let mut reader = reader_state
        .get_mut(&mut world)
        .expect("Audio command reader should fetch");
    let cmds: Vec<_> = reader.read().collect();

    assert!(
        cmds.iter()
            .all(|cmd| !matches!(cmd, AudioCmd::StopAllMusic)),
        "Scene switching should not stop all music automatically"
    );
}

// ---------------------------------------------------------------------------
// Test 12: gui_callback fn pointer roundtrips through RenderSceneTable unchanged
// ---------------------------------------------------------------------------

#[test]
fn gui_callback_stored_and_retrieved_via_render_scene_table() {
    fn my_gui(_: &mut GuiCtx) {}

    let mut table = RenderSceneTable::default();
    table.0.insert(
        "editor".to_string(),
        SceneRender {
            gui_callback: Some(my_gui as GuiCallback),
            world_draw_callback: None,
        },
    );

    let render = table.get("editor").expect("scene must be present");
    let stored = render.gui_callback.expect("gui_callback must be Some");
    assert_eq!(
        stored as *const () as usize, my_gui as *const () as usize,
        "fn pointer must survive insertion/retrieval unchanged"
    );
}

// ---------------------------------------------------------------------------
// Test 13: a scene with a GUI callback enters normally, and its callback
// resolves for the active scene name (as render_system resolves it against
// RenderActiveScene).
// ---------------------------------------------------------------------------

#[test]
fn scene_with_gui_callback_enters_correctly() {
    fn editor_gui(_: &mut GuiCtx) {}

    let mut world = scene_world(&["editor"]);
    let mut render_table = RenderSceneTable::default();
    render_table.0.insert(
        "editor".to_string(),
        SceneRender {
            gui_callback: Some(editor_gui as GuiCallback),
            world_draw_callback: None,
        },
    );

    enter_play(&mut world);

    assert_eq!(SceneLog::take(&mut world), ["enter editor"]);
    let active = active_scene(&world).expect("active_scene must be set");
    let render = render_table
        .get(active)
        .expect("render entry must be present");
    assert_eq!(
        render.gui_callback.map(|gui| gui as *const () as usize),
        Some(editor_gui as *const () as usize),
        "gui_callback must be resolvable for the active scene"
    );
}
