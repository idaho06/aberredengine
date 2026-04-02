//! Integration tests for the Rust `MenuRustCallback` feature.
//!
//! Validates that menu selection correctly follows the priority chain:
//! Lua callback → Rust callback → `MenuActions`.

use aberredengine::components::menu::{Menu, MenuAction, MenuActions, MenuRustCallback};
use aberredengine::events::audio::AudioCmd;
use aberredengine::events::menu::MenuSelectionEvent;
use aberredengine::resources::gameconfig::GameConfig;
use aberredengine::resources::gamestate::{GameState, NextGameState};
#[cfg(feature = "lua")]
use aberredengine::resources::lua_runtime::LuaRuntime;
use aberredengine::resources::camerafollowconfig::CameraFollowConfig;
use aberredengine::resources::postprocessshader::PostProcessShader;
use aberredengine::resources::systemsstore::SystemsStore;
use aberredengine::resources::texturestore::TextureStore;
use aberredengine::resources::worldsignals::WorldSignals;
use aberredengine::resources::worldtime::WorldTime;
use aberredengine::systems::GameCtx;
use aberredengine::systems::menu::menu_selection_observer;
use bevy_ecs::observer::Observer;
use bevy_ecs::prelude::*;

/// Set up a minimal world with all resources needed by `GameCtx` and
/// `menu_selection_observer`.
fn setup_world() -> World {
    let mut world = World::new();
    world.insert_resource(WorldSignals::default());
    world.insert_resource(WorldTime::default());
    world.insert_resource(GameState::new());
    world.insert_resource(NextGameState::new());
    world.insert_resource(SystemsStore::new());
    world.insert_resource(Messages::<AudioCmd>::default());
    world.insert_resource(TextureStore::default());
    world.insert_resource(GameConfig::default());
    world.init_resource::<PostProcessShader>();
    world.insert_resource(CameraFollowConfig::default());
    #[cfg(feature = "lua")]
    world.insert_non_send_resource(LuaRuntime::new().expect("LuaRuntime::new() failed in test"));
    world
}

/// Spawn a menu entity with the given labels and return its entity ID.
fn spawn_menu(world: &mut World, labels: &[(&str, &str)]) -> Entity {
    world
        .spawn(Menu::new(
            labels,
            raylib::prelude::Vector2::zero(),
            "test_font",
            16.0,
            20.0,
            true,
        ))
        .id()
}

// ---------------------------------------------------------------------------
// Test: Rust callback is invoked with correct arguments
// ---------------------------------------------------------------------------

// We use a thread-local to capture callback args since fn pointers can't close
// over environment.
thread_local! {
    static CALLBACK_ARGS: std::cell::RefCell<Option<(u64, String, usize)>> = const { std::cell::RefCell::new(None) };
}

fn test_callback(_entity: Entity, item_id: &str, item_index: usize, ctx: &mut GameCtx) {
    // Record that we were called with the right args
    CALLBACK_ARGS.with(|args| {
        *args.borrow_mut() = Some((0, item_id.to_string(), item_index));
    });
    // Prove we have ECS access: set a signal
    ctx.world_signals.set_flag("callback_fired");
}

#[test]
fn rust_callback_invoked_with_correct_args() {
    let mut world = setup_world();

    let menu_entity = world
        .spawn(
            Menu::new(
                &[("play", "Play"), ("options", "Options"), ("quit", "Quit")],
                raylib::prelude::Vector2::zero(),
                "test_font",
                16.0,
                20.0,
                true,
            )
            .with_on_rust_callback(test_callback as MenuRustCallback),
        )
        .id();

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    // Clear any prior state
    CALLBACK_ARGS.with(|args| *args.borrow_mut() = None);

    // Trigger selection of the second item ("options", index 1)
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "options".to_string(),
    });

    // Verify callback was called with correct item_id and index
    CALLBACK_ARGS.with(|args| {
        let captured = args.borrow();
        let (_, item_id, item_index) = captured.as_ref().expect("callback was not called");
        assert_eq!(item_id, "options");
        assert_eq!(*item_index, 1);
    });

    // Verify the callback had ECS access (set a signal)
    assert!(world.resource::<WorldSignals>().has_flag("callback_fired"));
}

// ---------------------------------------------------------------------------
// Test: MenuActions still work when no callback is set (regression)
// ---------------------------------------------------------------------------

#[test]
fn menu_actions_work_without_callback() {
    let mut world = setup_world();

    let menu_entity = spawn_menu(&mut world, &[("start", "Start"), ("quit", "Quit")]);
    world.entity_mut(menu_entity).insert(
        MenuActions::new()
            .with("start", MenuAction::SetScene("level01".to_string()))
            .with("quit", MenuAction::QuitGame),
    );

    // Register a dummy switch_scene system so the SetScene action doesn't panic
    let switch_id = world.register_system(|| {});
    {
        let mut store = world.resource_mut::<SystemsStore>();
        store.insert("switch_scene", switch_id);
    }

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    // Trigger "start" → should SetScene
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "start".to_string(),
    });

    assert_eq!(
        world.resource::<WorldSignals>().get_string("scene"),
        Some(&"level01".to_string())
    );
}

// ---------------------------------------------------------------------------
// Test: Rust callback takes priority over MenuActions
// ---------------------------------------------------------------------------

thread_local! {
    static PRIORITY_CALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn priority_callback(_entity: Entity, _item_id: &str, _index: usize, ctx: &mut GameCtx) {
    PRIORITY_CALLED.with(|c| c.set(true));
    ctx.world_signals.set_flag("rust_callback_ran");
}

#[test]
fn rust_callback_takes_priority_over_menu_actions() {
    let mut world = setup_world();

    let menu_entity = world
        .spawn(
            Menu::new(
                &[("start", "Start")],
                raylib::prelude::Vector2::zero(),
                "test_font",
                16.0,
                20.0,
                true,
            )
            .with_on_rust_callback(priority_callback as MenuRustCallback),
        )
        .id();

    // Also attach MenuActions (which should be skipped)
    world
        .entity_mut(menu_entity)
        .insert(MenuActions::new().with("start", MenuAction::SetScene("level01".to_string())));

    // Register switch_scene system in case it's reached (it shouldn't be)
    let switch_id = world.register_system(|| {});
    {
        let mut store = world.resource_mut::<SystemsStore>();
        store.insert("switch_scene", switch_id);
    }

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    PRIORITY_CALLED.with(|c| c.set(false));

    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "start".to_string(),
    });

    // Rust callback should have run
    assert!(PRIORITY_CALLED.with(|c| c.get()));
    assert!(
        world
            .resource::<WorldSignals>()
            .has_flag("rust_callback_ran")
    );

    // MenuActions should NOT have run (scene should not be set)
    assert_eq!(world.resource::<WorldSignals>().get_string("scene"), None);
}

// ---------------------------------------------------------------------------
// Test: No-op when neither callback nor actions are present
// ---------------------------------------------------------------------------

#[test]
fn no_callback_no_actions_does_nothing() {
    let mut world = setup_world();

    // Menu with no callback and no MenuActions
    let menu_entity = spawn_menu(&mut world, &[("item1", "Item 1")]);

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    // This should not panic — it just logs a warning
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "item1".to_string(),
    });

    // No signals should have been set
    assert!(!world.resource::<WorldSignals>().has_flag("callback_fired"));
    assert_eq!(world.resource::<WorldSignals>().get_string("scene"), None);
}

// ---------------------------------------------------------------------------
// Test: Callback receives correct index for first and last items
// ---------------------------------------------------------------------------

thread_local! {
    static INDEX_CAPTURE: std::cell::RefCell<Vec<usize>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn index_callback(_entity: Entity, _item_id: &str, item_index: usize, _ctx: &mut GameCtx) {
    INDEX_CAPTURE.with(|v| v.borrow_mut().push(item_index));
}

#[test]
fn callback_receives_correct_indices() {
    let mut world = setup_world();

    let menu_entity = world
        .spawn(
            Menu::new(
                &[("a", "A"), ("b", "B"), ("c", "C")],
                raylib::prelude::Vector2::zero(),
                "test_font",
                16.0,
                20.0,
                true,
            )
            .with_on_rust_callback(index_callback as MenuRustCallback),
        )
        .id();

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    INDEX_CAPTURE.with(|v| v.borrow_mut().clear());

    // Trigger each item
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "a".to_string(),
    });
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "c".to_string(),
    });
    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "b".to_string(),
    });

    INDEX_CAPTURE.with(|v| {
        let captured = v.borrow();
        assert_eq!(*captured, vec![0, 2, 1]);
    });
}

// ---------------------------------------------------------------------------
// Test: Unknown item_id defaults to index 0
// ---------------------------------------------------------------------------

thread_local! {
    static UNKNOWN_INDEX: std::cell::Cell<usize> = const { std::cell::Cell::new(999) };
}

fn unknown_callback(_entity: Entity, _item_id: &str, item_index: usize, _ctx: &mut GameCtx) {
    UNKNOWN_INDEX.with(|c| c.set(item_index));
}

#[test]
fn unknown_item_id_defaults_to_index_zero() {
    let mut world = setup_world();

    let menu_entity = world
        .spawn(
            Menu::new(
                &[("a", "A"), ("b", "B")],
                raylib::prelude::Vector2::zero(),
                "test_font",
                16.0,
                20.0,
                true,
            )
            .with_on_rust_callback(unknown_callback as MenuRustCallback),
        )
        .id();

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    UNKNOWN_INDEX.with(|c| c.set(999));

    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "nonexistent".to_string(),
    });

    assert_eq!(UNKNOWN_INDEX.with(|c| c.get()), 0);
}

// ---------------------------------------------------------------------------
// Test: Lua callback takes priority over Rust callback (lua feature only)
// ---------------------------------------------------------------------------

#[cfg(feature = "lua")]
thread_local! {
    static RUST_CB_CALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(feature = "lua")]
fn lua_priority_callback(_entity: Entity, _item_id: &str, _index: usize, _ctx: &mut GameCtx) {
    RUST_CB_CALLED.with(|c| c.set(true));
}

#[cfg(feature = "lua")]
#[test]
fn lua_callback_takes_priority_over_rust_callback() {
    let mut world = setup_world();

    // Register a Lua function the observer will find
    {
        let lua_rt = world.non_send_resource::<LuaRuntime>();
        lua_rt
            .lua()
            .load("function on_menu_select(menu, item_id, index) end")
            .exec()
            .expect("failed to load Lua function");
    }

    let menu_entity = world
        .spawn(
            Menu::new(
                &[("play", "Play")],
                raylib::prelude::Vector2::zero(),
                "test_font",
                16.0,
                20.0,
                true,
            )
            .with_on_select_callback("on_menu_select")
            .with_on_rust_callback(lua_priority_callback as MenuRustCallback),
        )
        .id();

    world.spawn(Observer::new(menu_selection_observer));
    world.flush();

    RUST_CB_CALLED.with(|c| c.set(false));

    world.trigger(MenuSelectionEvent {
        menu: menu_entity,
        item_id: "play".to_string(),
    });

    // Lua had priority → Rust callback should NOT have been called
    assert!(
        !RUST_CB_CALLED.with(|c| c.get()),
        "Rust callback should be skipped when Lua callback is set"
    );
}
