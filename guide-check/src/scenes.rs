// Approach A — SceneManager: scene callback signatures and scene switch
mod scene_callbacks {
    use aberredengine::prelude::*;

    // Called once when the scene becomes active (logic thread)
    fn enter(ctx: &mut GameCtx) { /* spawn entities, set signals */ }

    // Called once per sim tick while the scene is active (logic thread)
    fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) { /* per-tick logic */ }

    // Called once when leaving the scene, before entities are despawned (logic thread)
    fn exit(ctx: &mut GameCtx) { /* cleanup */ }

    // Called every render frame to draw ImGui widgets — Rust-only, optional, RENDER thread
    // Signature must match: fn(&Ui, &SignalSnapshot, &mut SignalIntents, &TextureStore, &FontStore, &AppState)
    fn my_gui(
        ui: &imgui::Ui,
        signals: &SignalSnapshot,
        intents: &mut SignalIntents,
        textures: &TextureStore,
        fonts: &FontStore,
        app_state: &AppState,
    ) { /* draw widgets, queue signal writes, read typed state */ }

    // Called every render frame inside begin_mode2D in world space — Rust-only, optional, RENDER thread
    // Signature must match: fn(&mut dyn WorldDraw, &Camera2D, &ScreenSize, &AppState, &SignalSnapshot)
    fn my_world_draw(
        draw: &mut dyn WorldDraw,
        camera: &Camera2D,
        screen: &ScreenSize,
        app_state: &AppState,
        signals: &SignalSnapshot,
    ) { /* draw world overlays, read camera/screen/app state */ }

    fn descriptor() -> aberredengine::engine_app::SceneDescriptor { // GLUE
        aberredengine::engine_app::SceneDescriptor { // GLUE
            on_enter: enter, on_update: Some(update), on_exit: Some(exit), // GLUE
            gui_callback: Some(my_gui), world_draw_callback: Some(my_world_draw), // GLUE
        } // GLUE
    } // GLUE

    mod switch { // GLUE
    use super::*; // GLUE
    fn some_condition() -> bool { true } // GLUE

    fn update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
        if some_condition() {
            ctx.world_signals.request_scene("level01");
        }
    }
    } // GLUE
}

// ImGui GUI callback (Rust-only)
mod imgui_gui_callback {

    use aberredengine::prelude::*;

    #[derive(Clone)]
    struct EditorPanelState {
        active_tool: String,
    }

    fn editor_gui(
        ui: &imgui::Ui,
        _signals: &SignalSnapshot,
        intents: &mut SignalIntents,
        _textures: &TextureStore,
        _fonts: &FontStore,
        app_state: &AppState,
    ) {
        // Read typed state written by on_update or ECS systems
        let tool = app_state
            .get::<EditorPanelState>()
            .map(|s| s.active_tool.as_str())
            .unwrap_or("select");

        ui.text(format!("Active tool: {tool}"));

        if let Some(_mb) = ui.begin_main_menu_bar()
            && let Some(_file) = ui.begin_menu("File")
            && ui.menu_item("Save")
        {
            intents.set_flag("gui:action:file:save"); // consumed by on_update next sim tick
        }
    }

    fn editor_update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
        // Every insert bumps AppState's generation, so the snapshot clones it again.
        // A real game inserts only when the value changes (see the AppState API section).
        ctx.app_state.insert(EditorPanelState {
            active_tool: "place".to_string(),
        });

        if ctx.world_signals.take_flag("gui:action:file:save") {
            // handle save
        }
    }

    fn editor_enter(_ctx: &mut GameCtx) {} // GLUE

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new() // GLUE
    .add_scene("editor", SceneDescriptor {
        on_enter:     editor_enter,
        on_update:    Some(editor_update),
        on_exit:      None,
        gui_callback: Some(editor_gui),
        world_draw_callback: None,
    })
    } // GLUE
}

// World-space draw callback (Rust-only)
mod world_space_draw_callback {
    fn editor_enter(_: &mut GameCtx) {} // GLUE
    fn editor_update(_: &mut GameCtx, _: f32, _: &InputState) {} // GLUE
    fn editor_gui(_: &imgui::Ui, _: &SignalSnapshot, _: &mut SignalIntents, _: &TextureStore, _: &FontStore, _: &AppState) {} // GLUE

    use aberredengine::prelude::*;

    fn editor_world_draw(
        draw: &mut dyn WorldDraw,
        _camera: &Camera2D,
        _screen: &ScreenSize,
        _app_state: &AppState,
        _signals: &SignalSnapshot,
    ) {
        draw.draw_line_v(
            Vec2::new(-32.0, 0.0),
            Vec2::new(32.0, 0.0),
            Color::GREEN,
        );
        draw.draw_line(-16, -16, 16, 16, Color::YELLOW);
    }

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new() // GLUE
    .add_scene("editor", SceneDescriptor {
        on_enter:            editor_enter,
        on_update:           Some(editor_update),
        on_exit:             None,
        gui_callback:        Some(editor_gui),
        world_draw_callback: Some(editor_world_draw),
    })
    } // GLUE
}

// Approach B — Raw hooks
mod approach_b_raw_hooks {
    fn my_setup() {} // GLUE
    fn my_enter_play() {} // GLUE
    fn my_switch_scene() {} // GLUE

    use aberredengine::prelude::*;

    fn main() -> Result<(), EngineError> {
        EngineBuilder::new()
            .config("config.ini")
            .title("My Game")
            .on_setup(my_setup)
            .on_enter_play(my_enter_play)
            .add_system(my_update)
            .on_switch_scene(my_switch_scene)
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
    fn editor_scene() -> SceneDescriptor { unimplemented!() } // GLUE

    fn main() { // GLUE
    EngineBuilder::new()
        .config("config.ini")
        .on_setup(load_assets)
        .add_system(tilemap_load_system)   // checks a signal each tick, then queues a load
        .add_system(tilemap_save_system)   // independent second system
        .add_scene("editor", editor_scene())
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
            id: path.clone(),
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

    // From a scene callback (via GameCtx):
    fn my_enter(ctx: &mut GameCtx) {
        ctx.commands.trigger(TilemapLoaded { path: "maps/intro.json".into() });
    }
}

// Scene-scoped (transient) observers
mod scene_scoped_observers {
    #[derive(Event)] // GLUE
    struct TileSelectedEvent; // GLUE

    use aberredengine::prelude::*;

    fn editor_enter(ctx: &mut GameCtx) {
        // This observer lives only until the next scene switch.
        // The scene switch despawns every non-Persistent entity, this observer included.
        ctx.commands.spawn(Observer::new(on_tile_selected));
    }

    fn on_tile_selected(trigger: On<TileSelectedEvent>, /* params */) {
        // only fires while the editor scene is active
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

// Game lifecycle: switch_scene poll with raw hooks
mod switch_on_flag {
    use aberredengine::prelude::*; // GLUE
    fn my_switch_scene() {} // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;
    use aberredengine::core::resources::systemsstore::{self as hook_keys, SystemsStore};

    fn switch_on_flag(
        mut signals: ResMut<WorldSignals>,
        systems: Res<SystemsStore>,
        mut commands: Commands,
    ) {
        if signals.take_flag(sk::SWITCH_SCENE)
            && let Some(switch_scene) = systems.get(hook_keys::SWITCH_SCENE)
        {
            commands.run_system(*switch_scene);
        }
    }

    EngineBuilder::new()
        .on_switch_scene(my_switch_scene)
        .add_system(switch_on_flag)
        // …
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

    // In a GameCtx callback
    fn roll_damage(ctx: &mut GameCtx) -> i32 {
        ctx.sim_rng.0.i32(5..=10)
    }
}
