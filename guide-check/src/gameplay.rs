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
        builder.add_observer(on_spawner_timer).add_observer(end_invulnerability) // GLUE
    } // GLUE

    #[derive(Component)]
    struct Invulnerable;

    // Three seconds of invulnerability for the player
    fn grant_invulnerability(commands: &mut Commands, player: Entity) {
        commands.entity(player).insert((Invulnerable, Timer::once(3.0)));
    }

    // Registered once with `EngineBuilder::add_observer`
    fn end_invulnerability(
        ev: On<TimerFired>,
        invulnerable: Query<(), With<Invulnerable>>,
        mut commands: Commands,
    ) {
        if invulnerable.contains(ev.entity) {
            commands.entity(ev.entity).remove::<Invulnerable>();
        }
    }
}

// 7.2 Phase State Machines
mod phase_state_machines {

    use aberredengine::prelude::*;

    #[derive(Component)]
    struct Player;

    fn spawn_player(mut commands: Commands) {
        commands.spawn((
            Player,
            MapPosition::new(100.0, 200.0),
            RigidBody::default(),
            // ... sprite, collider, etc ...
            Phase::new("idle"),
        ));
    }

    // Registered with `EngineBuilder::add_system`; runs once per sim tick
    fn player_phases(
        mut players: Query<(&mut Phase, &mut RigidBody), With<Player>>,
        input: Res<InputState>,
    ) {
        for (mut phase, mut rb) in &mut players {
            let next = match phase.current.as_str() {
                "idle" if input.action(InputAction::Action1).just_pressed => {
                    rb.velocity.y = -400.0;
                    "jumping"
                }
                "jumping" if rb.velocity.y > 0.0 => "falling",
                "falling" if rb.velocity.y == 0.0 => "idle",
                _ => continue,
            };
            phase.next = Some(next.into());
        }
    }

    fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
        builder.add_system(player_phases).add_observer(on_player_phase_entered) // GLUE
    } // GLUE

    // Registered once with `EngineBuilder::add_observer`
    fn on_player_phase_entered(
        ev: On<PhaseEntered>,
        players: Query<(), With<Player>>,
        mut audio: MessageWriter<AudioCmd>,
    ) {
        if players.contains(ev.entity) && &*ev.name == "jumping" {
            audio.write(AudioCmd::PlayFx { id: "jump".into() });
        }
    }
}

// 7.3 Collision Rules
mod collision_rules {
    use aberredengine::prelude::*; // GLUE

    fn spawn_rule(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands
        .spawn((
            CollisionRule::new("ball", "brick"),
            Persistent, // survive scene switches
        ))
        .observe(ball_brick_collision);
    } // GLUE

    use aberredengine::prelude::*;

    fn ball_brick_collision(
        hit: On<Collided>,
        mut commands: Commands,
        mut rigid_bodies: Query<&mut RigidBody>,
        mut audio: MessageWriter<AudioCmd>,
    ) {
        // Despawn the brick
        commands.entity(hit.b).despawn();
        audio.write(AudioCmd::PlayFx { id: "break".into() });

        // Reflect ball velocity based on collision side
        if let Ok(mut rb) = rigid_bodies.get_mut(hit.a) {
            for side in hit.sides_a.iter() {
                match side {
                    BoxSide::Top | BoxSide::Bottom => rb.velocity.y = -rb.velocity.y,
                    BoxSide::Left | BoxSide::Right => rb.velocity.x = -rb.velocity.x,
                }
            }
        }
    }
}

// 7.4 Menus
mod menus {
    use aberredengine::prelude::*; // GLUE

    fn spawn_with_actions(mut commands: Commands) { // GLUE
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

    commands.spawn((menu, actions));
    } // GLUE

    fn spawn_with_observer(mut commands: Commands, menu: Menu) { // GLUE
    use aberredengine::prelude::*;

    fn on_menu_select(ev: On<MenuSelected>, mut signals: ResMut<WorldSignals>) {
        match ev.item_id.as_str() {
            "start" => signals.request_scene("level01"),
            "quit" => signals.request_quit(),
            _ => {}
        }
    }

    commands.spawn(menu).observe(on_menu_select);
    } // GLUE

    use aberredengine::prelude::*;

    fn enter(_: On<SceneEntered>, mut commands: Commands) {
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

        commands.spawn((menu, actions));
    }
}

// 7.5 Animation Finished Event
mod animation_finished_event {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    fn on_anim_done(
        trigger: On<AnimationFinished>,
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
