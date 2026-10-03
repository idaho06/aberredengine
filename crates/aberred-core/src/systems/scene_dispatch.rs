//! Scene dispatch systems for Rust-native scene management.
//!
//! This module provides the systems and types behind [`SceneManager`]: every
//! Rust game runs in scenes (its own, or the implicit
//! [`MAIN_SCENE`](crate::resources::signal_keys::MAIN_SCENE)).
//!
//! - [`scene_switch_system`] — engine-owned scene transition: [`SceneExited`] → teardown →
//!   [`SceneEntered`]
//! - [`insert_scene_manager`] — inserts [`SceneManager`] and spawns the persistent [`SceneName`] entity the scene events target
//! - [`in_scene`] — run condition: true while the named scene is active
//! - [`scene_switch_poll`] — polls `WorldSignals["switch_scene"]` and triggers a scene transition
//! - [`scene_enter_play`] — one-shot system that seeds the initial scene and triggers the first switch
//!
//! Scene behavior is ECS: observers of [`SceneEntered`]/[`SceneExited`] and systems
//! gated on [`in_scene`]. A scene's render callbacks live in `aberred-render`'s
//! `SceneRender`, keyed by the same scene name.
//!
//! # Related
//!
//! - [`crate::resources::scenemanager::SceneManager`] — the registry resource
//! - `aberredengine::EngineBuilder::add_scene` — builder method for registration

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;
use log::{debug, error, info};

use crate::components::persistent::{Persistent, SceneCleanup};
use crate::components::scene::SceneName;
use crate::events::scene::{SceneEntered, SceneExited};
use crate::math::{Color, Vec2};
use crate::resources::appstate::AppState;
use crate::resources::camera2d::Camera2D;
use crate::resources::group::TrackedGroups;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::signal_keys as sk;
use crate::resources::systemsstore as hook_keys;
use crate::resources::systemsstore::SystemsStore;
use crate::resources::worldsignals::{SignalSnapshot, WorldSignals};

/// Minimal world-space drawing interface for `WorldDrawCallback`.
/// Uses concrete types only so the callback stays object-safe.
pub trait WorldDraw {
    fn draw_line_v(&mut self, start: Vec2, end: Vec2, color: Color);
    fn draw_line_ex(&mut self, start_pos: Vec2, end_pos: Vec2, thick: f32, color: Color);
    fn draw_line_dashed(
        &mut self,
        start_pos: Vec2,
        end_pos: Vec2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    );
    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color);
}

/// Forwards through a mutable reference, so a `&mut &mut dyn WorldDraw` (e.g. the
/// `draw` binding from destructuring a `&mut WorldDrawCtx`) passes where a
/// `&mut dyn WorldDraw` is expected.
impl<T: WorldDraw + ?Sized> WorldDraw for &mut T {
    fn draw_line_v(&mut self, start: Vec2, end: Vec2, color: Color) {
        (**self).draw_line_v(start, end, color);
    }
    fn draw_line_ex(&mut self, start_pos: Vec2, end_pos: Vec2, thick: f32, color: Color) {
        (**self).draw_line_ex(start_pos, end_pos, thick, color);
    }
    fn draw_line_dashed(
        &mut self,
        start_pos: Vec2,
        end_pos: Vec2,
        dash_size: i32,
        space_size: i32,
        color: Color,
    ) {
        (**self).draw_line_dashed(start_pos, end_pos, dash_size, space_size, color);
    }
    fn draw_line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, color: Color) {
        (**self).draw_line(x1, y1, x2, y2, color);
    }
}

// The `WorldDraw` impl over raylib draw handles lives in
// `aberred-render`'s `systems::math` as `RaylibWorldDraw` — a newtype
// wrapper, not a blanket impl, since `aberred-render` owns neither
// `WorldDraw` (defined here, in core) nor `RaylibDraw` (defined in the
// `raylib` crate) — the orphan rule blocks a blanket impl across two
// foreign types.

// `GuiCallback` (the ImGui-drawing callback type) lives in
// `aberred-render`'s `resources::scene_table` alongside `SceneRender` —
// core cannot name `ImguiUi`/`TextureStore`/`FontStore`.

/// Called every frame inside `begin_mode2D` in camera-transformed world space.
pub type WorldDrawCallback = fn(&mut WorldDrawCtx);

/// What a [`WorldDrawCallback`] can draw with and read.
///
/// Everything but `draw` is a read-only render-side copy: `signals` mirrors the
/// render-side `GuiCallback`, which likewise holds no live `&WorldSignals`.
/// New fields can be added without breaking callbacks, since only the engine
/// constructs this.
#[non_exhaustive]
pub struct WorldDrawCtx<'a> {
    /// Line-drawing interface, in camera-transformed world space.
    pub draw: &'a mut dyn WorldDraw,
    /// The camera this frame is drawn with.
    pub camera: &'a Camera2D,
    /// The internal game resolution.
    pub screen: &'a ScreenSize,
    /// The latest snapshot of the logic world's [`AppState`].
    pub app_state: &'a AppState,
    /// The latest snapshot of the logic world's signals.
    pub signals: &'a SignalSnapshot,
}

impl<'a> WorldDrawCtx<'a> {
    /// Engine-internal: the render thread builds this once per frame.
    #[doc(hidden)]
    pub fn new(
        draw: &'a mut dyn WorldDraw,
        camera: &'a Camera2D,
        screen: &'a ScreenSize,
        app_state: &'a AppState,
        signals: &'a SignalSnapshot,
    ) -> Self {
        Self {
            draw,
            camera,
            screen,
            app_state,
            signals,
        }
    }
}

// ---------------------------------------------------------------------------
// scene_switch_system — engine-owned scene transition
// ---------------------------------------------------------------------------

/// What [`scene_switch_system`] needs between its two scene events.
type SceneTeardown = (
    Commands<'static, 'static>,
    SceneCleanup<'static, 'static>,
    ResMut<'static, WorldSignals>,
    ResMut<'static, TrackedGroups>,
    ResMut<'static, SceneManager>,
);

/// Handles scene transitions for [`SceneManager`]-based games.
///
/// This system is registered into [`SystemsStore`] under `"switch_scene"` when
/// the developer uses `aberredengine::EngineBuilder::add_scene`.
///
/// Reads the target from `WorldSignals["scene"]` and checks it first: an
/// unregistered name logs an error and leaves the current scene untouched.
/// Otherwise, when a scene is active, it tears that scene down: it triggers
/// [`SceneExited`] while the scene's entities are still alive, despawns
/// non-[`Persistent`] entities, resets tracked groups to the persistent ones and
/// records the old scene's name under [`sk::PREVIOUS_SCENE`]. Entering the first
/// scene tears nothing down, so what was spawned before it (during `Setup`)
/// survives. Then it makes the new scene active and triggers [`SceneEntered`], so
/// entities its observers spawn belong to the new scene.
pub fn scene_switch_system(world: &mut World, state: &mut SystemState<SceneTeardown>) {
    debug!("scene_switch_system: System called!");

    let scene_name: Arc<str> = world
        .resource::<WorldSignals>()
        .get_string(sk::SCENE)
        .unwrap_or(sk::DEFAULT_SCENE)
        .into();
    let scene_manager = world.resource::<SceneManager>();
    let Some(entered) = scene_manager.scene_entity(&scene_name) else {
        error!(
            "scene_switch_system: No scene registered for '{}'; staying in the current scene. Registered scenes: {:?}",
            scene_name,
            scene_manager.scene_names()
        );
        return;
    };
    let exited = scene_manager
        .active_scene
        .as_deref()
        .and_then(|prev| Some((Arc::<str>::from(prev), scene_manager.scene_entity(prev)?)));

    if let Some((prev, scene)) = &exited {
        world.trigger(SceneExited {
            scene: *scene,
            name: prev.clone(),
            next: scene_name.clone(),
        });
    }
    let previous = exited.map(|(prev, _)| prev);

    let (mut commands, scene_cleanup, mut world_signals, mut tracked_groups, mut scene_manager) =
        state.get_mut(world).expect(
            "the logic world holds the WorldSignals, TrackedGroups and SceneManager resources",
        );
    if let Some(prev) = &previous {
        scene_cleanup.despawn_all(&mut commands);

        // Clear entity registrations for despawned (non-persistent) entities
        world_signals.clear_non_persistent_entities(&scene_cleanup.persistent_set());

        tracked_groups.reset_to_persistent();
        world_signals.clear_group_counts();
        world_signals.set_string(sk::PREVIOUS_SCENE, &**prev);
    }

    info!("scene_switch_system: Entering scene '{}'", scene_name);
    scene_manager.active_scene = Some(scene_name.to_string());
    state.apply(world);

    world.trigger(SceneEntered {
        scene: entered,
        name: scene_name,
        previous,
    });
}

/// Inserts the [`SceneManager`] resource with `scenes` registered, spawning one
/// [`Persistent`] scene entity carrying [`SceneName`] per scene, which
/// [`SceneEntered`]/[`SceneExited`] target ([`SceneManager::scene_entity`]).
pub fn insert_scene_manager(
    world: &mut World,
    scenes: impl IntoIterator<Item = String>,
    initial_scene: Option<String>,
) {
    let mut scene_manager = SceneManager::new();
    scene_manager.initial_scene = initial_scene;
    for name in scenes {
        let entity = world
            .spawn((SceneName(Arc::from(name.as_str())), Persistent))
            .id();
        scene_manager.insert(name, entity);
    }
    world.insert_resource(scene_manager);
}

/// Run condition: true while `name` is the active [`SceneManager`] scene.
///
/// False when no [`SceneManager`] exists (no `.add_scene()`, or a Lua game).
///
/// ```ignore
/// builder.add_system_if(hud, in_scene("level01"))
/// ```
pub fn in_scene(name: &'static str) -> impl FnMut(Option<Res<SceneManager>>) -> bool + Clone {
    move |scene_manager| {
        scene_manager.is_some_and(|scenes| scenes.active_scene.as_deref() == Some(name))
    }
}

/// Polls the `"switch_scene"` flag in [`WorldSignals`] and runs the
/// scene switch system when set.
///
/// Added to the sim schedule for every game without Lua (all of them have
/// scenes: their own, or the implicit `"main"`).
pub fn scene_switch_poll(
    mut commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
) {
    if world_signals.take_flag(sk::SWITCH_SCENE) {
        commands.run_system(
            *systems_store
                .get(hook_keys::SWITCH_SCENE)
                .expect("the scene manager registers the 'switch_scene' system"),
        );
    }
}

// ---------------------------------------------------------------------------
// scene_enter_play — one-shot bootstrap
// ---------------------------------------------------------------------------

/// One-shot system registered as `"enter_play"` for SceneManager-based games.
///
/// Seeds `WorldSignals["scene"]` with the initial scene name (stored in
/// [`SceneManager`]) and then runs the `switch_scene` system.
pub fn scene_enter_play(
    mut commands: Commands,
    mut world_signals: ResMut<WorldSignals>,
    systems_store: Res<SystemsStore>,
    scene_manager: Res<SceneManager>,
) {
    let initial = scene_manager
        .initial_scene
        .as_ref()
        .cloned()
        .expect("SceneManager.initial_scene not set; validate_builder should have caught this");

    world_signals.set_string(sk::SCENE, initial);

    commands.run_system(
        *systems_store
            .get(hook_keys::SWITCH_SCENE)
            .expect("the scene manager registers the 'switch_scene' system"),
    );
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::insert_game_ctx_resources;
    use bevy_ecs::system::{IntoObserverSystem, RunSystemOnce};

    #[derive(Default)]
    struct RecordedLines(Vec<(Vec2, Vec2)>);

    impl WorldDraw for RecordedLines {
        fn draw_line_v(&mut self, start: Vec2, end: Vec2, _: Color) {
            self.0.push((start, end));
        }
        fn draw_line_ex(&mut self, _: Vec2, _: Vec2, _: f32, _: Color) {}
        fn draw_line_dashed(&mut self, _: Vec2, _: Vec2, _: i32, _: i32, _: Color) {}
        fn draw_line(&mut self, _: i32, _: i32, _: i32, _: i32, _: Color) {}
    }

    fn draw_axis(d: &mut dyn WorldDraw) {
        d.draw_line_v(Vec2::ZERO, Vec2::X, Color::WHITE);
    }

    /// The destructured `draw` binding is a `&mut &mut dyn WorldDraw`; it must still
    /// pass straight to helpers that take `&mut dyn WorldDraw`.
    fn overlay(ctx: &mut WorldDrawCtx) {
        let WorldDrawCtx { draw, .. } = ctx;
        draw_axis(draw);
    }

    #[test]
    fn destructured_draw_handle_passes_to_helpers() {
        let mut lines = RecordedLines::default();
        let (camera, screen) = (Camera2D::default(), ScreenSize { w: 1, h: 1 });
        let (app_state, signals) = (AppState::default(), SignalSnapshot::default());

        overlay(&mut WorldDrawCtx::new(
            &mut lines, &camera, &screen, &app_state, &signals,
        ));

        assert_eq!(lines.0, [(Vec2::ZERO, Vec2::X)]);
    }

    /// World with `menu` active (one scene entity, one tracked group) and
    /// `WorldSignals[scene]` set to `target`.
    fn world_in_menu_switching_to(target: &str) -> (World, Entity) {
        let mut world = World::new();
        insert_game_ctx_resources(&mut world);
        insert_scene_manager(&mut world, ["menu".into(), "level".into()], None);
        world.resource_mut::<SceneManager>().active_scene = Some("menu".to_owned());
        let mut groups = TrackedGroups::default();
        groups.add_group("enemies");
        world.insert_resource(groups);
        world
            .resource_mut::<WorldSignals>()
            .set_string(sk::SCENE, target);
        let menu_entity = world.spawn_empty().id();
        (world, menu_entity)
    }

    /// A global observer that survives scene switches, spawned the way
    /// `EngineBuilder::add_observer` does.
    fn add_global_observer<E: Event, B: Bundle, M>(
        world: &mut World,
        observer: impl IntoObserverSystem<E, B, M>,
    ) {
        world.spawn((Observer::new(observer), Persistent));
    }

    /// Scene events seen by the test observers, in trigger order.
    #[derive(Resource, Default)]
    struct SeenEvents(Vec<String>);

    fn record_scene_events(world: &mut World) {
        world.init_resource::<SeenEvents>();
        add_global_observer(
            world,
            |ev: On<SceneExited>, mut seen: ResMut<SeenEvents>| {
                seen.0.push(format!("exit {} -> {}", ev.name, ev.next));
            },
        );
        add_global_observer(
            world,
            |ev: On<SceneEntered>, mut seen: ResMut<SeenEvents>| {
                seen.0.push(format!(
                    "enter {} from {:?}",
                    ev.name,
                    ev.previous.as_deref()
                ));
            },
        );
    }

    #[test]
    fn switch_triggers_exited_then_entered_with_names() {
        let (mut world, _) = world_in_menu_switching_to("level");
        record_scene_events(&mut world);
        world.run_system_once(scene_switch_system).unwrap();

        assert_eq!(
            world.resource::<SeenEvents>().0,
            ["exit menu -> level", "enter level from Some(\"menu\")"]
        );
        assert_eq!(
            world
                .resource::<WorldSignals>()
                .get_string(sk::PREVIOUS_SCENE),
            Some("menu")
        );
    }

    #[test]
    fn scene_events_target_the_scene_entity() {
        let (mut world, _) = world_in_menu_switching_to("level");
        let scenes = world.resource::<SceneManager>();
        let (menu, level) = (
            scenes.scene_entity("menu").unwrap(),
            scenes.scene_entity("level").unwrap(),
        );
        assert_eq!(world.get::<SceneName>(level).map(|n| &*n.0), Some("level"));
        assert!(world.get::<Persistent>(level).is_some());
        world.init_resource::<SeenEvents>();
        world
            .entity_mut(menu)
            .observe(|_: On<SceneExited>, mut seen: ResMut<SeenEvents>| {
                seen.0.push("menu exited".into());
            });
        world
            .entity_mut(level)
            .observe(|_: On<SceneEntered>, mut seen: ResMut<SeenEvents>| {
                seen.0.push("level entered".into());
            });

        world.run_system_once(scene_switch_system).unwrap();

        assert_eq!(
            world.resource::<SeenEvents>().0,
            ["menu exited", "level entered"]
        );
    }

    #[test]
    fn exit_observer_sees_the_old_scene_alive() {
        let (mut world, menu_entity) = world_in_menu_switching_to("level");
        world.init_resource::<SeenEvents>();
        add_global_observer(
            &mut world,
            move |_: On<SceneExited>, q: Query<Entity>, mut seen: ResMut<SeenEvents>| {
                seen.0.push(format!("alive {}", q.contains(menu_entity)));
            },
        );

        world.run_system_once(scene_switch_system).unwrap();

        assert_eq!(world.resource::<SeenEvents>().0, ["alive true"]);
        assert!(
            world.get_entity(menu_entity).is_err(),
            "despawned afterwards"
        );
    }

    #[derive(Component)]
    struct SpawnedOnEnter;

    #[test]
    fn enter_observer_spawns_survive_the_switch() {
        let (mut world, _) = world_in_menu_switching_to("level");
        add_global_observer(&mut world, |_: On<SceneEntered>, mut commands: Commands| {
            commands.spawn(SpawnedOnEnter);
        });

        world.run_system_once(scene_switch_system).unwrap();

        let spawned = world.query::<&SpawnedOnEnter>().iter(&world).count();
        assert_eq!(spawned, 1);
    }

    #[test]
    fn first_scene_entered_has_no_previous() {
        let (mut world, _) = world_in_menu_switching_to("level");
        world.resource_mut::<SceneManager>().active_scene = None;
        record_scene_events(&mut world);

        world.run_system_once(scene_switch_system).unwrap();

        assert_eq!(world.resource::<SeenEvents>().0, ["enter level from None"]);
    }

    /// Teardown runs only when a scene is left: entering the first scene keeps
    /// what was spawned before it (e.g. during Setup) and its tracked groups.
    #[test]
    fn first_scene_entered_keeps_what_was_spawned_before_it() {
        let (mut world, spawned_before) = world_in_menu_switching_to("level");
        world.resource_mut::<SceneManager>().active_scene = None;

        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.get_entity(spawned_before).is_ok());
        assert!(world.resource::<TrackedGroups>().has_group("enemies"));
        assert_eq!(
            world.resource::<SceneManager>().active_scene.as_deref(),
            Some("level")
        );
    }

    #[test]
    fn unknown_target_triggers_no_scene_event() {
        let (mut world, _) = world_in_menu_switching_to("nope");
        record_scene_events(&mut world);

        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.resource::<SeenEvents>().0.is_empty());
    }

    #[test]
    fn in_scene_holds_only_for_the_active_scene() {
        let (mut world, _) = world_in_menu_switching_to("level");
        let check =
            |world: &mut World, name: &'static str| world.run_system_once(in_scene(name)).unwrap();
        assert!(check(&mut world, "menu"));
        assert!(!check(&mut world, "level"));

        world.run_system_once(scene_switch_system).unwrap();
        assert!(!check(&mut world, "menu"));
        assert!(check(&mut world, "level"));
    }

    #[test]
    fn in_scene_is_false_without_a_scene_manager() {
        let mut world = World::new();
        assert!(!world.run_system_once(in_scene("menu")).unwrap());
    }

    #[test]
    fn switch_to_a_registered_scene_tears_down_and_enters() {
        let (mut world, menu_entity) = world_in_menu_switching_to("level");
        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.get_entity(menu_entity).is_err());
        assert!(!world.resource::<TrackedGroups>().has_group("enemies"));
        assert_eq!(
            world.resource::<SceneManager>().active_scene.as_deref(),
            Some("level")
        );
    }

    #[test]
    fn switch_to_an_unknown_scene_keeps_the_current_scene() {
        let (mut world, menu_entity) = world_in_menu_switching_to("nope");
        world.run_system_once(scene_switch_system).unwrap();

        assert!(world.get_entity(menu_entity).is_ok(), "entities kept");
        assert!(world.resource::<TrackedGroups>().has_group("enemies"));
        assert_eq!(
            world.resource::<SceneManager>().active_scene.as_deref(),
            Some("menu")
        );
    }
}
