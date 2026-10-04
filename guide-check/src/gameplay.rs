// 7.1 Timers
mod timers {

    use aberredengine::prelude::*;

    fn spawn_repeating(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    // Spawn an entity with a 2-second repeating timer and a per-entity observer
    commands
        .spawn((MapPosition::new(0.0, 0.0), Timer::new(2.0)))
        .observe(on_timer_fired);

    fn on_timer_fired(ev: On<TimerFired>, mut signals: ResMut<WorldSignals>) {
        // This fires every 2 seconds; ev.entity is the timer's entity
        signals.set_string("timer_count", "fired!".to_string());
    }
    } // GLUE

    #[derive(Component)]
    struct Spawner;

    fn on_spawner_timer(
        ev: On<TimerFired>,
        spawners: Query<&MapPosition, With<Spawner>>,
        mut commands: Commands,
    ) {
        let Ok(pos) = spawners.get(ev.entity) else { return };
        commands.spawn(MapPosition::new(pos.pos.x, pos.pos.y));
    }

    fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
        builder.add_observer(on_spawner_timer) // GLUE
    } // GLUE

    fn spawn_one_shot(mut commands: Commands) { // GLUE
        commands.spawn(Timer::new(5.0)).observe(one_shot); // GLUE
    } // GLUE

    fn one_shot(ev: On<TimerFired>, mut commands: Commands, mut audio: MessageWriter<AudioCmd>) {
        audio.write(AudioCmd::PlayFx { id: "explosion".into() });
        commands.entity(ev.entity).remove::<Timer>();
    }
}

// 7.2 Phase State Machines
mod phase_state_machines {
    use rustc_hash::FxHashMap; // GLUE

    use aberredengine::prelude::*;

    // Called when entering a phase. Return Some("phase") to immediately chain-transition.
    type PhaseEnterFn = fn(Entity, &mut GameCtx, &InputState) -> Option<String>;

    // Called once per sim tick while in a phase. Return Some("phase") to transition.
    type PhaseUpdateFn = fn(Entity, &mut GameCtx, &InputState, f32) -> Option<String>;

    // Called when exiting a phase. No return — the transition is already committed.
    type PhaseExitFn = fn(Entity, &mut GameCtx);

    fn same_as_engine( // GLUE
        e: PhaseEnterFn, u: PhaseUpdateFn, x: PhaseExitFn, // GLUE
    ) -> PhaseCallbackFns { // GLUE
        PhaseCallbackFns { on_enter: Some(e), on_update: Some(u), on_exit: Some(x) } // GLUE
    } // GLUE

    fn spawn_player(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;
    use rustc_hash::FxHashMap;

    let mut phases = FxHashMap::default();

    phases.insert("idle".to_string(), PhaseCallbackFns {
        on_enter: Some(idle_enter),
        on_update: Some(idle_update),
        on_exit: None,
    });

    phases.insert("jumping".to_string(), PhaseCallbackFns {
        on_enter: Some(jumping_enter),
        on_update: Some(jumping_update),
        on_exit: Some(jumping_exit),
    });

    phases.insert("falling".to_string(), PhaseCallbackFns {
        on_enter: None,
        on_update: Some(falling_update),
        on_exit: None,
    });

    ctx.commands.spawn((
        MapPosition::new(100.0, 200.0),
        // ... sprite, rigidbody, etc ...
        Phase::new("idle", phases),
    ));
    } // GLUE

    fn falling_only() { // GLUE
    let mut phases = FxHashMap::default(); // GLUE
    // Equivalent to the "falling" entry above — only on_update is set
    phases.insert("falling".to_string(), PhaseCallbackFns {
        on_update: Some(falling_update),
        ..Default::default()
    });
    } // GLUE

    use aberredengine::prelude::*;

    fn idle_enter(_entity: Entity, _ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        None // stay in idle
    }

    fn idle_update(entity: Entity, ctx: &mut GameCtx, input: &InputState, _dt: f32) -> Option<String> {
        if input.action(InputAction::Action1).just_pressed {
            // Apply jump velocity
            if let Ok(mut rb) = ctx.rigid_bodies.get_mut(entity) {
                rb.velocity.y = -400.0;
            }
            return Some("jumping".to_string());
        }
        None
    }

    fn jumping_enter(_entity: Entity, ctx: &mut GameCtx, _input: &InputState) -> Option<String> {
        ctx.audio.write(AudioCmd::PlayFx { id: "jump".into() });
        None
    }

    fn jumping_update(entity: Entity, ctx: &mut GameCtx, _input: &InputState, _dt: f32) -> Option<String> {
        if let Ok(rb) = ctx.rigid_bodies.get(entity)
            && rb.velocity.y > 0.0
        {
            return Some("falling".to_string());
        }
        None
    }

    fn jumping_exit(_entity: Entity, _ctx: &mut GameCtx) {
        // cleanup if needed
    }

    fn falling_update(entity: Entity, ctx: &mut GameCtx, _input: &InputState, _dt: f32) -> Option<String> {
        // Transition back to idle when landing (detected by some condition)
        if let Ok(rb) = ctx.rigid_bodies.get(entity)
            && rb.velocity.y == 0.0
        {
            return Some("idle".to_string());
        }
        None
    }
}

// 7.3 Collision Rules
mod collision_rules {

    use aberredengine::prelude::*;

    type CollisionCallback = fn(Entity, Entity, &BoxSides, &BoxSides, &mut GameCtx);

    fn same_as_engine(f: CollisionCallback) -> aberredengine::core::components::collision::CollisionCallback { f } // GLUE

    mod usage { // GLUE
    use super::*; // GLUE

    fn spawn_rule(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        CollisionRule::rust("ball", "brick", ball_brick_collision),
        Persistent, // survive scene switches
    ));
    } // GLUE

    use aberredengine::prelude::*;

    fn ball_brick_collision(
        ball: Entity,
        brick: Entity,
        ball_sides: &BoxSides,
        _brick_sides: &BoxSides,
        ctx: &mut GameCtx,
    ) {
        // Despawn the brick
        ctx.commands.entity(brick).despawn();
        ctx.audio.write(AudioCmd::PlayFx { id: "break".into() });

        // Reflect ball velocity based on collision side
        if let Ok(mut rb) = ctx.rigid_bodies.get_mut(ball) {
            for side in ball_sides.iter() {
                match side {
                    BoxSide::Top | BoxSide::Bottom => rb.velocity.y = -rb.velocity.y,
                    BoxSide::Left | BoxSide::Right => rb.velocity.x = -rb.velocity.x,
                }
            }
        }
    }
    } // GLUE
}

// 7.4 Menus
mod menus {
    use aberredengine::prelude::*; // GLUE

    fn spawn_with_actions(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    let menu = Menu::new(
        &[("start", "Start Game"), ("options", "Options"), ("quit", "Quit")],
        Vec2::new(100.0, 80.0), // origin position
        "arcade",                        // font key
        24.0,                            // font size
        30.0,                            // item spacing (pixels)
        true,                            // use_screen_space
    );

    let actions = MenuActions::new()
        .with("start", MenuAction::SetScene("level01".to_string()))
        .with("options", MenuAction::SetScene("options_menu".to_string()))
        .with("quit", MenuAction::QuitGame);

    ctx.commands.spawn((menu, actions));
    } // GLUE

    fn spawn_with_callback(ctx: &mut GameCtx, menu: Menu) { // GLUE
    use aberredengine::core::components::menu::MenuRustCallback;
    use aberredengine::prelude::*;

    fn on_menu_select(menu_entity: Entity, item_id: &str, item_index: usize, ctx: &mut GameCtx) {
        match item_id {
            "start" => {
                ctx.world_signals.request_scene("level01");
            }
            "quit" => {
                ctx.world_signals.request_quit();
            }
            _ => {}
        }
    }

    ctx.commands.spawn((
        menu.with_on_rust_callback(on_menu_select),
    ));
    } // GLUE

    use aberredengine::prelude::*;

    fn enter(ctx: &mut GameCtx) {
        let menu = Menu::new(
            &[("play", "Play"), ("quit", "Quit")],
            Vec2::new(200.0, 150.0),
            "arcade",
            32.0,
            40.0,
            true,
        )
        .with_colors(Color::GRAY, Color::WHITE)
        .with_selection_sound("menu_move");

        let actions = MenuActions::new()
            .with("play", MenuAction::SetScene("level01".to_string()))
            .with("quit", MenuAction::QuitGame);

        ctx.commands.spawn((menu, actions));
    }
}

// 7.5 Animation Finished Event
mod animation_finished_event {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    fn on_anim_done(
        trigger: On<AnimationFinishedEvent>,
        mut commands: Commands,
    ) {
        // Despawn the entity whose animation just finished
        commands.entity(trigger.event().entity).despawn();
    }

    // Register in main:
    EngineBuilder::new()
        .add_observer(on_anim_done)
        // …
    } // GLUE
}

// 7.6 Tween Finished Event
mod tween_finished_event {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    fn on_move_tween_done(
        trigger: On<TweenFinishedEvent<MapPosition>>,
        mut commands: Commands,
    ) {
        // Despawn the entity whose move tween just finished
        commands.entity(trigger.event().entity).despawn();
    }

    // Register in main:
    EngineBuilder::new()
        .add_observer(on_move_tween_done)
        // …
    } // GLUE
}
