// Multi-scene games: scene observer/system signatures
mod scene_callbacks {
    use aberredengine::prelude::*;

    // Observes SceneEntered: runs each time the scene becomes active, after the old scene is torn down (logic thread)
    fn enter(_: On<SceneEntered>, mut commands: Commands) { /* spawn entities, set signals */ }

    // Scene system: runs once per sim tick while the scene is active (logic thread)
    fn update(time: Res<WorldTime>, input: Res<InputState>) { /* per-tick logic */ }

    // Observes SceneExited: runs when the scene is left, before its entities are despawned (logic thread)
    fn exit(_: On<SceneExited>, mut signals: ResMut<WorldSignals>) { /* save state */ }

    fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
        builder.on_scene_enter("a", enter).add_scene_system("a", update).on_scene_exit("a", exit) // GLUE
    } // GLUE
}

// ImGui GUI callback (Rust-only)
mod imgui_gui_callback {

    use aberredengine::prelude::*;

    #[derive(Clone)]
    struct EditorPanelState {
        active_tool: String,
    }

    fn editor_gui(ctx: &mut GuiCtx) {
        let GuiCtx { ui, intents, app_state, .. } = ctx;

        // Read typed state written by ECS systems
        let tool = app_state
            .get::<EditorPanelState>()
            .map(|s| s.active_tool.as_str())
            .unwrap_or("select");

        ui.text(format!("Active tool: {tool}"));

        if let Some(_mb) = ui.begin_main_menu_bar()
            && let Some(_file) = ui.begin_menu("File")
            && ui.menu_item("Save")
        {
            intents.set_flag("gui:action:file:save"); // consumed by editor_update next sim tick
        }
    }

    fn editor_update(mut app_state: ResMut<AppState>, mut signals: ResMut<WorldSignals>) {
        // Every insert bumps AppState's generation, so the snapshot clones it again.
        // A real game inserts only when the value changes (see the AppState API section).
        app_state.insert(EditorPanelState {
            active_tool: "place".to_string(),
        });

        if signals.take_flag("gui:action:file:save") {
            // handle save
        }
    }

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new() // GLUE
    .add_scene_system("editor", editor_update)
    .add_scene_gui("editor", editor_gui)
    } // GLUE
}

// World-space draw callback (Rust-only)
mod world_space_draw_callback {
    fn editor_gui(_: &mut GuiCtx) {} // GLUE

    use aberredengine::prelude::*;

    fn editor_world_draw(ctx: &mut WorldDrawCtx) {
        // `..` is required: WorldDrawCtx is #[non_exhaustive]
        let WorldDrawCtx { draw, .. } = ctx;
        draw_origin_cross(draw);
    }

    fn draw_origin_cross(draw: &mut dyn WorldDraw) {
        draw.draw_line_v(
            Vec2::new(-32.0, 0.0),
            Vec2::new(32.0, 0.0),
            Color::GREEN,
        );
        draw.draw_line(-16, -16, 16, 16, Color::YELLOW);
    }

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new() // GLUE
    .add_scene_gui("editor", editor_gui)
    .add_scene_world_draw("editor", editor_world_draw)
    } // GLUE
}

// Single-scene games
mod single_scene_games {
    fn my_setup() {} // GLUE
    fn my_enter(_: aberredengine::prelude::On<aberredengine::prelude::SceneEntered>) {} // GLUE

    use aberredengine::prelude::*;

    fn main() -> Result<(), EngineError> {
        EngineBuilder::new()
            .config("config.ini")
            .title("My Game")
            .on_setup(my_setup)
            .on_scene_enter("main", my_enter)
            .add_system(my_update)
            .try_run()
    }

    use aberredengine::prelude::*;

    fn my_update(signals: ResMut<WorldSignals>, input: Res<InputState>) {
        if input.action(InputAction::Action1).just_pressed {
            // ...
        }
    }
}

// `.add_system(system)` — multiple per-sim-tick systems
mod add_system {
    use aberredengine::prelude::*; // GLUE
    fn load_assets() {} // GLUE
    fn tilemap_save_system() {} // GLUE

    fn main() { // GLUE
    EngineBuilder::new()
        .config("config.ini")
        .on_setup(load_assets)
        .add_system(tilemap_load_system)   // checks a signal each tick, then queues a load
        .add_system(tilemap_save_system)   // independent second system
        .add_scene("editor")
        .initial_scene("editor")
        .try_run()
        .expect("engine startup failed");
    } // GLUE

    use aberredengine::prelude::*;

    fn tilemap_load_system(
        mut world_signals: ResMut<WorldSignals>,
        mut asset_cmds: MessageWriter<RenderAssetCmd>,
    ) {
        let Some(path) = world_signals.remove_string("pending_load_path") else {
            return; // nothing to do this tick
        };
        asset_cmds.write(RenderAssetCmd::Texture {
            key: path.clone(),
            path,
            filter: TextureFilter::Nearest,
        });
        // spawn tile entities, etc. — the texture itself loads asynchronously
    }
}

// `.configure_schedule(closure)` — full ordering control
mod configure_schedule {
    use aberredengine::prelude::*; // GLUE
    fn undo_system() {} // GLUE

    fn main() { // GLUE
    use aberredengine::core::systems::movement::movement;
    use aberredengine::core::systems::camera_follow::camera_follow_system;
    use aberredengine::prelude::*;

    EngineBuilder::new()
        .configure_schedule(|schedule| {
            schedule.add_systems(
                undo_system
                    .run_if(state_is_playing)
                    .after(movement)
                    .before(camera_follow_system),
            );
        })
        // …
        .try_run()
        .expect("engine startup failed");
    } // GLUE
}

// `.add_observer(observer_fn)` — persistent event observers
mod add_observer {

    use aberredengine::prelude::*;

    #[derive(Event)]
    struct TilemapLoaded {
        pub path: String,
    }

    use aberredengine::prelude::*;

    fn on_tilemap_loaded(
        trigger: On<TilemapLoaded>,
        mut world_signals: ResMut<WorldSignals>,
    ) {
        let path = &trigger.event().path;
        world_signals.set_string("last_loaded_tilemap", path.clone());
        log::info!("Tilemap loaded: {}", path);
    }

    fn main() { // GLUE
    EngineBuilder::new()
        .add_observer(on_tilemap_loaded)
        // …
        .try_run()
        .expect("engine startup failed");
    } // GLUE

    // From a Bevy ECS system:
    fn my_system(mut commands: Commands) {
        commands.trigger(TilemapLoaded { path: "maps/level01.json".into() });
    }

    // From a scene observer:
    fn my_enter(_: On<SceneEntered>, mut commands: Commands) {
        commands.trigger(TilemapLoaded { path: "maps/intro.json".into() });
    }
}

// Scene-scoped systems and observers
mod scene_scoped_systems_and_observers {
    use aberredengine::prelude::*;

    // Runs once per sim tick, only while "level01" is active.
    fn level_tick(mut signals: ResMut<WorldSignals>) {
        signals.set_flag("level01_ticking");
    }

    // Runs while either level is active.
    fn hud_update(time: Res<WorldTime>) {
        let _elapsed = time.elapsed;
    }

    // Fires each time "level01" becomes active, after the previous scene is torn down.
    fn spawn_level(_: On<SceneEntered>, mut commands: Commands) {
        commands.spawn((Group::new("player"), MapPosition::new(100.0, 200.0)));
    }

    // Fires each time "level01" is left, while its entities are still alive.
    fn save_score(_: On<SceneExited>, players: Query<&Signals, With<Group>>) {
        let _count = players.iter().count();
    }

    // A global observer fires for every scene; the event carries the names.
    fn log_scene(ev: On<SceneEntered>) {
        log::info!("entered {} from {:?}", ev.name, ev.previous);
    }

    fn register(builder: EngineBuilder) -> EngineBuilder {
        builder
            .add_scene_system("level01", level_tick)
            .add_system_if(hud_update, in_scene("level01").or_else(in_scene("level02")))
            .on_scene_enter("level01", spawn_level)
            .on_scene_exit("level01", save_score)
            .add_observer(log_scene)
    }
}

// Scene-scoped (transient) observers
mod scene_scoped_observers {
    #[derive(Event)] // GLUE
    struct TileSelectedEvent; // GLUE

    use aberredengine::prelude::*;

    fn editor_enter(_: On<SceneEntered>, mut commands: Commands) {
        // This observer lives only until the next scene switch.
        // The scene switch despawns every non-Persistent entity, this observer included.
        commands.spawn(Observer::new(on_tile_selected));
    }

    fn on_tile_selected(trigger: On<TileSelectedEvent>, /* params */) {
        // only fires while the editor scene is active
    }
}

// Triggering scene transitions
mod triggering_scene_transitions {
    use aberredengine::prelude::*; // GLUE
    fn player_reached_exit() -> bool { false } // GLUE

    fn update(mut signals: ResMut<WorldSignals>) {
        if player_reached_exit() {
            signals.request_scene("level02");
        }
    }
}

// Persistent entities
mod persistent_entities {
    use aberredengine::prelude::*; // GLUE

    fn enter(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
        ScreenPosition::new(10.0, 10.0),
        DynamicText::new("0", "arcade", 16.0, Color::WHITE),
        SignalBinding::new("score").with_format("Score: {}"),
        ZIndex(100.0),
        Persistent,  // survives scene switches
    ));
    } // GLUE

    use aberredengine::core::components::persistent::SceneCleanup;

    fn my_cleanup(scene_cleanup: SceneCleanup, mut commands: Commands) {
        scene_cleanup.despawn_all(&mut commands);
    }
}

// Group tracking across scenes
mod group_tracking_across_scenes {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new()
        .track_group("enemies")
        .track_group("bricks")
        // …
    } // GLUE
}

// The sim tick and `dt`
mod sim_tick_and_dt {
    use aberredengine::prelude::*; // GLUE

    fn update(time: Res<WorldTime>, input: Res<InputState>) {
        let dt = time.delta; // the fixed sim period, 1.0 / hz, scaled by time_scale
        // input = current action state (just_pressed, active, just_released)
    }
}

// Determinism and replay
mod determinism_and_replay {
    use aberredengine::prelude::*; // GLUE

    fn seeded() { // GLUE
    EngineBuilder::new()
        .config("config.ini")
        .deterministic(42)
        // …
        .try_run()
        .expect("engine startup failed");
    } // GLUE

    fn record_and_replay() { // GLUE
    // Record this session's input to a file:
    EngineBuilder::new()
        .deterministic(42)
        .record_replay("session01.replay", env!("CARGO_PKG_VERSION"))
        // …
        .try_run()
        .expect("engine startup failed");

    // Later, replay it deterministically instead of reading live input:
    EngineBuilder::new()
        .play_replay("session01.replay")
        // seed comes from the file's header — do not also call .deterministic()
        // …
        .try_run()
        .expect("engine startup failed");
    } // GLUE
}

// Determinism and replay: drawing random numbers
mod sim_rng {
    use aberredengine::prelude::*;

    // In a system
    fn pick_spawn_point(mut rng: ResMut<SimRng>) {
        let x = rng.0.f32() * 640.0; // 0.0..640.0
        let lane = rng.0.usize(0..4); // 0, 1, 2 or 3
        let flip = rng.0.bool();
    }
}
