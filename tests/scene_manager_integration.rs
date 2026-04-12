//! Integration tests for the `SceneManager` / scene plugin pattern.
//!
//! Validates that `SceneManager`-based games correctly dispatch `on_enter`,
//! `on_update`, and `on_exit` callbacks, and that conflict checks in
//! `EngineBuilder` fire as expected.

use aberredengine::resources::gameconfig::GameConfig;
use aberredengine::resources::group::TrackedGroups;
use aberredengine::resources::input::InputState;
use aberredengine::resources::camerafollowconfig::CameraFollowConfig;
use aberredengine::resources::input_bindings::InputBindings;
use aberredengine::resources::postprocessshader::PostProcessShader;
use aberredengine::resources::scenemanager::SceneManager;
use aberredengine::resources::systemsstore::SystemsStore;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::worldtime::WorldTime;
use aberredengine::systems::GameCtx;
use aberredengine::systems::scene_dispatch::{
    GuiCallback, SceneDescriptor, scene_enter_play, scene_switch_poll, scene_switch_system,
    scene_update_system,
};
use bevy_ecs::prelude::*;
use bevy_ecs::system::RunSystemOnce;

use aberredengine::components::persistent::Persistent;
use aberredengine::events::audio::AudioCmd;
use aberredengine::resources::gamestate::{GameState, NextGameState};

/// Set up a minimal world with all resources needed by `GameCtx`,
/// `scene_switch_system`, and `scene_update_system`.
fn setup_world() -> World {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());
    world.insert_resource(WorldTime::default().with_time_scale(1.0));
    world.insert_resource(TrackedGroups::default());
    world.insert_resource(TextureStore::default());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_resource(Messages::<AudioCmd>::default());
    world.insert_resource(InputState::default());
    world.insert_resource(GameConfig::default());
    world.init_resource::<PostProcessShader>();
    world.insert_resource(CameraFollowConfig::default());
    world.insert_resource(InputBindings::default());
    world
}

/// Helper to register scene_switch_system in SystemsStore.
fn register_switch_system(world: &mut World) {
    let sys_id = world.register_system(scene_switch_system);
    world.entity_mut(sys_id.entity()).insert(Persistent);
    let mut store = world.resource_mut::<SystemsStore>();
    store.insert("switch_scene", sys_id);
}

// ---------------------------------------------------------------------------
// Thread-local tracking for callback invocations
// ---------------------------------------------------------------------------

thread_local! {
    static ENTER_LOG: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    static UPDATE_LOG: std::cell::RefCell<Vec<(String, f32)>> = const { std::cell::RefCell::new(Vec::new()) };
    static EXIT_LOG: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn clear_logs() {
    ENTER_LOG.with(|v| v.borrow_mut().clear());
    UPDATE_LOG.with(|v| v.borrow_mut().clear());
    EXIT_LOG.with(|v| v.borrow_mut().clear());
}

// -- Menu scene callbacks --

fn menu_enter(ctx: &mut GameCtx) {
    ENTER_LOG.with(|v| v.borrow_mut().push("menu".to_string()));
    ctx.world_signals.set_flag("menu_entered");
}

fn menu_update(_ctx: &mut GameCtx, dt: f32, _input: &InputState) {
    UPDATE_LOG.with(|v| v.borrow_mut().push(("menu".to_string(), dt)));
}

fn menu_exit(ctx: &mut GameCtx) {
    EXIT_LOG.with(|v| v.borrow_mut().push("menu".to_string()));
    ctx.world_signals.set_flag("menu_exited");
}

// -- Level1 scene callbacks --

fn level1_enter(ctx: &mut GameCtx) {
    ENTER_LOG.with(|v| v.borrow_mut().push("level1".to_string()));
    ctx.world_signals.set_flag("level1_entered");
}

fn level1_update(_ctx: &mut GameCtx, dt: f32, _input: &InputState) {
    UPDATE_LOG.with(|v| v.borrow_mut().push(("level1".to_string(), dt)));
}

// -- Minimal scene (no update, no exit) --

fn minimal_enter(_ctx: &mut GameCtx) {
    ENTER_LOG.with(|v| v.borrow_mut().push("minimal".to_string()));
}

// ---------------------------------------------------------------------------
// Test 1: on_enter called for initial scene on enter_play
// ---------------------------------------------------------------------------

#[test]
fn initial_scene_on_enter_called() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: Some(menu_update),
            on_exit: Some(menu_exit),
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    register_switch_system(&mut world);

    // Run scene_enter_play — it sets WorldSignals["scene"] and runs switch_scene
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();

    // on_enter should have been called for "menu"
    ENTER_LOG.with(|v| {
        let log = v.borrow();
        assert_eq!(*log, vec!["menu"]);
    });
    assert!(world.resource::<WorldSignals>().has_flag("menu_entered"));

    // SceneManager should track "menu" as active
    let sm = world.resource::<SceneManager>();
    assert_eq!(sm.active_scene.as_deref(), Some("menu"));
}

// ---------------------------------------------------------------------------
// Test 2: on_exit called before on_enter on scene switch
// ---------------------------------------------------------------------------

#[test]
fn exit_called_before_enter_on_switch() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: Some(menu_exit),
            gui_callback: None,
        },
    );
    sm.insert(
        "level1",
        SceneDescriptor {
            on_enter: level1_enter,
            on_update: Some(level1_update),
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    register_switch_system(&mut world);

    // Enter initial scene
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();
    clear_logs();

    // Request scene switch to level1
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "level1".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    // on_exit("menu") should have been called BEFORE on_enter("level1")
    EXIT_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["menu"]);
    });
    ENTER_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["level1"]);
    });

    assert!(world.resource::<WorldSignals>().has_flag("menu_exited"));
    assert!(world.resource::<WorldSignals>().has_flag("level1_entered"));

    let sm = world.resource::<SceneManager>();
    assert_eq!(sm.active_scene.as_deref(), Some("level1"));
}

// ---------------------------------------------------------------------------
// Test 3: on_update called with correct dt
// ---------------------------------------------------------------------------

#[test]
fn on_update_called_with_dt() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: Some(menu_update),
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    // Set delta time
    {
        let mut wt = world.resource_mut::<WorldTime>();
        wt.delta = 0.016;
    }

    world.run_system_once(scene_update_system).unwrap();

    UPDATE_LOG.with(|v| {
        let log = v.borrow();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].0, "menu");
        assert!((log[0].1 - 0.016).abs() < f32::EPSILON);
    });
}

// ---------------------------------------------------------------------------
// Test 4: on_update: None scenes don't panic
// ---------------------------------------------------------------------------

#[test]
fn no_update_callback_does_not_panic() {
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("minimal".to_string());
    sm.insert(
        "minimal",
        SceneDescriptor {
            on_enter: minimal_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    // Should not panic
    world.run_system_once(scene_update_system).unwrap();
}

// ---------------------------------------------------------------------------
// Test 5: on_exit: None scenes don't panic on transition
// ---------------------------------------------------------------------------

#[test]
fn no_exit_callback_does_not_panic_on_switch() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("minimal".to_string());
    sm.insert(
        "minimal",
        SceneDescriptor {
            on_enter: minimal_enter,
            on_update: None,
            on_exit: None, // no exit callback
            gui_callback: None,
        },
    );
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    register_switch_system(&mut world);

    // Enter initial scene
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();
    clear_logs();

    // Switch to menu — should not panic even though minimal has no on_exit
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    // No exit should have been logged
    EXIT_LOG.with(|v| {
        assert!(v.borrow().is_empty());
    });
    // Enter should have been called for menu
    ENTER_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["menu"]);
    });
}

// ---------------------------------------------------------------------------
// Test 6: Switching to unregistered scene name → no panic
// ---------------------------------------------------------------------------

#[test]
fn unknown_scene_name_does_not_panic() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: Some(menu_exit),
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    world.insert_resource(TrackedGroups::default());

    register_switch_system(&mut world);

    // Request switch to a scene that doesn't exist
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "nonexistent".to_string());
    }

    // Should NOT panic — logs an error
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    // on_exit for "menu" should still have been called
    EXIT_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["menu"]);
    });
    // on_enter should NOT have been called (no scene found)
    ENTER_LOG.with(|v| {
        assert!(v.borrow().is_empty());
    });
}

// ---------------------------------------------------------------------------
// Test 7: Non-persistent entities are despawned on scene switch
// ---------------------------------------------------------------------------

#[test]
fn non_persistent_entities_despawned() {
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    register_switch_system(&mut world);

    // Spawn some entities — one persistent, two not
    world.spawn(Persistent);
    world.spawn(());
    world.spawn(());

    // 3 spawned + 1 system entity (switch_scene)
    let count_before: usize = world
        .query::<Entity>()
        .iter(&world)
        .filter(|_| true)
        .count();
    assert!(count_before >= 3);

    // Switch scene (stays on "menu" since that's the default)
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    // Only persistent entities should remain
    let non_persistent: Vec<Entity> = world
        .query_filtered::<Entity, Without<Persistent>>()
        .iter(&world)
        .collect();
    assert!(
        non_persistent.is_empty(),
        "Non-persistent entities should be despawned; found {}",
        non_persistent.len()
    );
}

// ---------------------------------------------------------------------------
// Test 8: SceneManager tracks active scene correctly through multiple switches
// ---------------------------------------------------------------------------

#[test]
fn active_scene_tracked_through_multiple_switches() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: Some(menu_exit),
            gui_callback: None,
        },
    );
    sm.insert(
        "level1",
        SceneDescriptor {
            on_enter: level1_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);

    register_switch_system(&mut world);

    // Enter initial scene
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();
    assert_eq!(
        world.resource::<SceneManager>().active_scene.as_deref(),
        Some("menu")
    );

    // Switch to level1
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "level1".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();
    assert_eq!(
        world.resource::<SceneManager>().active_scene.as_deref(),
        Some("level1")
    );

    // Switch back to menu
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();
    assert_eq!(
        world.resource::<SceneManager>().active_scene.as_deref(),
        Some("menu")
    );

    // Verify full enter/exit sequence
    ENTER_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["menu", "level1", "menu"]);
    });
    // level1 has no on_exit, so only menu's exit is logged (first switch only)
    EXIT_LOG.with(|v| {
        assert_eq!(*v.borrow(), vec!["menu"]);
    });
}

// ---------------------------------------------------------------------------
// Test 9: scene_switch_poll triggers transition when flag is set
// ---------------------------------------------------------------------------

#[test]
fn scene_switch_poll_triggers_transition() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: Some(menu_update),
            on_exit: Some(menu_exit),
            gui_callback: None,
        },
    );
    sm.insert(
        "level1",
        SceneDescriptor {
            on_enter: level1_enter,
            on_update: Some(level1_update),
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    // Enter initial scene
    world.run_system_once(scene_enter_play).unwrap();
    world.flush();
    clear_logs();

    // Simulate what a scene callback would do: set scene + flag
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "level1".to_string());
        ws.set_flag("switch_scene");
    }

    // Run scene_switch_poll (what the schedule would do)
    world.run_system_once(scene_switch_poll).unwrap();
    world.flush();

    // Flag should be cleared
    assert!(!world.resource::<WorldSignals>().has_flag("switch_scene"));

    // Scene transition should have been queued and executed
    assert_eq!(
        world.resource::<SceneManager>().active_scene.as_deref(),
        Some("level1")
    );
    EXIT_LOG.with(|v| assert_eq!(*v.borrow(), vec!["menu"]));
    ENTER_LOG.with(|v| assert_eq!(*v.borrow(), vec!["level1"]));
}

// ---------------------------------------------------------------------------
// Test 10: scene_switch_poll is a no-op when flag is absent
// ---------------------------------------------------------------------------

#[test]
fn scene_switch_poll_noop_without_flag() {
    clear_logs();
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("menu".to_string());
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    // No flag set — should not panic or trigger anything
    world.run_system_once(scene_switch_poll).unwrap();
    world.flush();

    // Nothing should have changed
    assert_eq!(
        world.resource::<SceneManager>().active_scene.as_deref(),
        Some("menu")
    );
    ENTER_LOG.with(|v| assert!(v.borrow().is_empty()));
}

// ---------------------------------------------------------------------------
// Test 11: Non-persistent registered entity is cleared on scene switch
// ---------------------------------------------------------------------------

#[test]
fn non_persistent_entity_registration_cleared_on_scene_switch() {
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    // Spawn a non-persistent entity and register it
    let player = world.spawn(()).id();
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_entity("player", player);
    }
    assert!(
        world
            .resource::<WorldSignals>()
            .get_entity("player")
            .is_some()
    );

    // Switch scene
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    assert!(
        world
            .resource::<WorldSignals>()
            .get_entity("player")
            .is_none(),
        "Non-persistent entity registration should be cleared on scene switch"
    );
}

// ---------------------------------------------------------------------------
// Test 12: Persistent registered entity survives scene switch
// ---------------------------------------------------------------------------

#[test]
fn persistent_entity_registration_survives_scene_switch() {
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    // Spawn a persistent entity and register it
    let cursor = world.spawn(Persistent).id();
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_entity("cursor", cursor);
    }

    // Switch scene
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    assert_eq!(
        world.resource::<WorldSignals>().get_entity("cursor"),
        Some(&cursor),
        "Persistent entity registration should survive scene switch"
    );
}

// ---------------------------------------------------------------------------
// Test 13: Mixed registrations — only non-persistent entries are cleared
// ---------------------------------------------------------------------------

#[test]
fn mixed_registrations_only_non_persistent_cleared_on_scene_switch() {
    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.active_scene = Some("menu".to_string());
    sm.insert(
        "menu",
        SceneDescriptor {
            on_enter: menu_enter,
            on_update: None,
            on_exit: None,
            gui_callback: None,
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    // Spawn one persistent and two non-persistent entities; register all three
    let cursor = world.spawn(Persistent).id();
    let player = world.spawn(()).id();
    let enemy = world.spawn(()).id();
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_entity("cursor", cursor);
        ws.set_entity("player", player);
        ws.set_entity("enemy", enemy);
    }

    // Switch scene
    {
        let mut ws = world.resource_mut::<WorldSignals>();
        ws.set_string("scene", "menu".to_string());
    }
    world.run_system_once(scene_switch_system).unwrap();
    world.flush();

    let ws = world.resource::<WorldSignals>();
    assert_eq!(
        ws.get_entity("cursor"),
        Some(&cursor),
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
// Test 14: gui_callback fn pointer roundtrips through SceneManager unchanged
// ---------------------------------------------------------------------------

#[test]
fn gui_callback_stored_and_retrieved_via_scene_manager() {
    fn my_gui(_ui: &::imgui::Ui, _signals: &mut WorldSignals, _tex: &TextureStore) {}

    let mut sm = SceneManager::new();
    sm.insert(
        "editor",
        SceneDescriptor {
            on_enter: minimal_enter,
            on_update: None,
            on_exit: None,
            gui_callback: Some(my_gui as GuiCallback),
        },
    );

    let desc = sm.get("editor").expect("scene must be present");
    let stored = desc.gui_callback.expect("gui_callback must be Some");
    assert_eq!(
        stored as *const () as usize, my_gui as *const () as usize,
        "fn pointer must survive insertion/retrieval unchanged"
    );
}

// ---------------------------------------------------------------------------
// Test 15: scene_enter_play with gui_callback — on_enter fires, callback accessible
// ---------------------------------------------------------------------------

#[test]
fn scene_with_gui_callback_enters_correctly() {
    clear_logs();
    fn editor_gui(_ui: &::imgui::Ui, _signals: &mut WorldSignals, _tex: &TextureStore) {}

    let mut world = setup_world();

    let mut sm = SceneManager::new();
    sm.initial_scene = Some("editor".to_string());
    sm.insert(
        "editor",
        SceneDescriptor {
            on_enter: menu_enter, // reuse menu_enter to check ENTER_LOG
            on_update: None,
            on_exit: None,
            gui_callback: Some(editor_gui as GuiCallback),
        },
    );
    world.insert_resource(sm);
    register_switch_system(&mut world);

    world.run_system_once(scene_enter_play).unwrap();
    world.flush();

    // on_enter must have fired
    ENTER_LOG.with(|v| {
        assert_eq!(
            *v.borrow(),
            vec!["menu"],
            "on_enter must fire for scene with gui_callback"
        );
    });

    // gui_callback must still be accessible on the active scene descriptor
    let sm = world.resource::<SceneManager>();
    let active = sm
        .active_scene
        .as_deref()
        .expect("active_scene must be set");
    let desc = sm.get(active).expect("descriptor must be present");
    assert!(
        desc.gui_callback.is_some(),
        "gui_callback must be preserved on the descriptor after scene activation"
    );
    assert_eq!(
        desc.gui_callback.unwrap() as *const () as usize,
        editor_gui as *const () as usize,
        "gui_callback fn pointer must be unchanged after scene activation"
    );
}
