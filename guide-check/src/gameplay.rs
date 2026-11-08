// Timers
mod timers {

    use aberredengine::prelude::*;

    fn spawn_repeating(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    // Spawn an entity with a 2-second repeating timer and a per-entity observer
    commands
        .spawn((WorldPosition::new(0.0, 0.0), Timer::new(2.0)))
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
        spawners: Query<&WorldPosition, With<Spawner>>,
        mut commands: Commands,
    ) {
        let Ok(pos) = spawners.get(ev.entity) else { return };
        commands.spawn(WorldPosition::new(pos.pos.x, pos.pos.y));
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

// Phase state machines
mod phase_state_machines {

    use aberredengine::prelude::*;

    #[derive(Component)]
    struct Player;

    fn spawn_player(mut commands: Commands) {
        commands.spawn((
            Player,
            WorldPosition::new(100.0, 200.0),
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

// Dialogues
mod dialogues {

    // Authoring a tree
    mod authoring {
        use aberredengine::prelude::*;
        use std::sync::Arc;

        // Registered with `EngineBuilder::on_setup`
        fn setup(mut dialogues: ResMut<DialogueStore>) -> Result {
            let tree = DialogueBuilder::new("greet")
                .node("greet")
                .speaker("Guard")
                .text("Halt! Who goes there?")
                .choice("A friend.", "friend")
                .choice("Here is my pass.", "pass")
                .when(DialogueCondition::flag("has_pass"))
                .effect(DialogueEffect::set_flag("pass_shown"))
                .choice_end("Never mind.")
                .label("rude")
                .node("friend")
                .speaker("Guard")
                .text("Hm. Move along.")
                .node("pass")
                .speaker("Guard")
                .text("Carry on.")
                .goto("friend")
                .build()?;
            dialogues.insert("guard", Arc::new(tree));
            Ok(())
        }

        fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
            builder.on_setup(setup) // GLUE
        } // GLUE
    }

    // Spawning a runner
    mod spawning {
        use aberredengine::prelude::*;
        use std::sync::Arc;

        fn start_guard_talk(dialogues: Res<DialogueStore>, mut commands: Commands) {
            let Some(tree) = dialogues.get("guard") else {
                return;
            };
            commands.spawn(DialogueRunner::new(Arc::clone(tree)));
        }

        fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
            builder.add_system(start_guard_talk) // GLUE
        } // GLUE
    }

    // Driving and reading a runner
    mod driving {
        use aberredengine::prelude::*;

        // Registered with `EngineBuilder::add_system`; runs once per sim tick
        fn drive_dialogue(mut runners: Query<&mut DialogueRunner>, input: Res<InputState>) {
            for mut runner in &mut runners {
                if !runner.is_active() || !input.action(InputAction::Action1).just_pressed {
                    continue;
                }
                if runner.has_choices() {
                    let first_open = runner
                        .choices()
                        .find(|choice| choice.available)
                        .map(|choice| choice.index);
                    if let Some(index) = first_open {
                        runner.choose(index);
                    }
                } else {
                    runner.advance();
                }
            }
        }

        fn dialogue_lines(runner: &DialogueRunner) -> Vec<String> {
            let mut lines = Vec::new();
            if let (Some(speaker), Some(text)) = (runner.speaker(), runner.text()) {
                lines.push(format!("{speaker}: {text}"));
            }
            for choice in runner.choices() {
                let lock = if choice.available { "" } else { " (locked)" };
                lines.push(format!("{}. {}{lock}", choice.index + 1, choice.text));
            }
            lines
        }

        fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
            builder.add_system(drive_dialogue) // GLUE
        } // GLUE
    }

    // Events
    mod events {
        use aberredengine::prelude::*;

        // Registered once with `EngineBuilder::add_observer`
        fn on_dialogue_choice(ev: On<DialogueChoiceSelected>, mut signals: ResMut<WorldSignals>) {
            if ev.labels.iter().any(|label| &**label == "rude") {
                signals.set_flag("guard_offended");
            }
        }

        fn on_dialogue_ended(ev: On<DialogueEnded>, mut commands: Commands) {
            if ev.reason != DialogueEndReason::Cancelled {
                commands.entity(ev.entity).despawn();
            }
        }

        fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
            builder
                .add_observer(on_dialogue_choice)
                .add_observer(on_dialogue_ended) // GLUE
        } // GLUE
    }

    // Placeholders
    mod placeholders {
        use aberredengine::prelude::*;

        #[derive(Component)]
        struct DialogueBox;

        // Registered with `EngineBuilder::add_system`; runs once per sim tick
        fn show_dialogue(
            runners: Query<(&DialogueRunner, Option<&Signals>)>,
            world: Res<WorldSignals>,
            mut boxes: Query<&mut DynamicText, With<DialogueBox>>,
        ) {
            let Ok(mut dialogue_box) = boxes.single_mut() else {
                return;
            };
            let Some((runner, stock)) = runners.iter().next() else {
                return;
            };
            let mut shown = String::new();
            if let Some(speaker) = runner.speaker() {
                shown.push_str(&format_signal_text(speaker, &*world, stock));
                shown.push_str(": ");
            }
            if let Some(text) = runner.text() {
                shown.push_str(&format_signal_text(text, &*world, stock));
            }
            for choice in runner.choices() {
                let label = format_signal_text(choice.text, &*world, stock);
                shown.push_str(&format!("\n{}. {label}", choice.index + 1));
            }
            if *dialogue_box.text != *shown {
                dialogue_box.set_text(&shown);
            }
        }

        fn register(builder: EngineBuilder) -> EngineBuilder { // GLUE
            builder.add_system(show_dialogue) // GLUE
        } // GLUE
    }

    #[cfg(test)]
    mod tests {
        /// Body of the first ```json fence after the `### Dialogues` heading of
        /// RUST-GAME-GUIDE.md.
        fn guide_dialogue_json() -> String {
            let guide = std::fs::read_to_string(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../RUST-GAME-GUIDE.md"
            ))
            .expect("read RUST-GAME-GUIDE.md");
            let lines: Vec<&str> = guide.lines().collect();
            let section = lines
                .iter()
                .position(|line| line.trim() == "### Dialogues")
                .expect("RUST-GAME-GUIDE.md has a `### Dialogues` section");
            let mut in_fence = false;
            let mut body = Vec::new();
            for line in &lines[section + 1..] {
                let trimmed = line.trim();
                if !in_fence {
                    if trimmed.starts_with("```json") {
                        in_fence = true;
                    }
                } else if trimmed.starts_with("```") {
                    return body.join("\n");
                } else {
                    body.push(*line);
                }
            }
            panic!("no ```json fence found after `### Dialogues` in RUST-GAME-GUIDE.md");
        }

        #[test]
        fn the_guide_json_loads() {
            let json = guide_dialogue_json();
            let path = std::env::temp_dir().join(format!(
                "guide-check-dialogue-{}.json",
                std::process::id()
            ));
            std::fs::write(&path, &json).expect("write the guide json to a temp file");
            let loaded = aberredengine::prelude::load_dialogue(&path);
            let _ = std::fs::remove_file(&path);
            let tree = loaded.expect("the guide's dialogue json loads and validates");
            for id in ["greet", "friend", "pass"] {
                assert!(
                    tree.idx_of(id).is_some(),
                    "the guide's dialogue tree has no node {id:?}"
                );
            }
        }
    }
}

// Collision rules
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
        hit: On<CollisionOngoing>,
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

// Collision rules: contact events
mod collision_contacts {
    use aberredengine::prelude::*;

    fn spawn_coin_rule(mut commands: Commands) {
        commands
            .spawn(CollisionRule::new("player", "coin"))
            .observe(pick_up_coin);
    }

    fn pick_up_coin(
        hit: On<CollisionStarted>,
        mut commands: Commands,
        mut signals: ResMut<WorldSignals>,
        mut audio: MessageWriter<AudioCmd>,
    ) {
        // Fires once per player-coin touch; try_despawn because two players
        // can touch the same coin on one tick
        commands.entity(hit.b).try_despawn();
        let score = signals.get_integer("score").unwrap_or(0);
        signals.set_integer("score", score + 1);
        audio.write(AudioCmd::PlayFx { id: "coin".into() });
    }
}

// Collision rules: leaving a zone
mod collision_contacts_ended {
    use aberredengine::prelude::*;

    fn leave_mud(ended: On<CollisionEnded>, mut bodies: Query<&mut RigidBody>) {
        if let Ok(mut rb) = bodies.get_mut(ended.a) {
            rb.friction = 0.0;
        }
    }
}

// Menus
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
        .with("start", MenuAction::RequestScene("level01".to_string()))
        .with("options", MenuAction::RequestScene("options_menu".to_string()))
        .with("quit", MenuAction::RequestQuit);

    commands.spawn((menu, actions));
    } // GLUE

    fn spawn_with_observer(mut commands: Commands, menu: Menu) { // GLUE
    use aberredengine::prelude::*;

    fn on_menu_select(
        ev: On<MenuSelected>,
        mut next_scene: ResMut<NextScene>,
        mut next_state: ResMut<NextGameState>,
    ) {
        match ev.item_id.as_str() {
            "start" => next_scene.set("level01"),
            "quit" => next_state.set(GameState::Quitting),
            _ => {}
        }
    }

    commands.spawn(menu).observe(on_menu_select);
    } // GLUE

    fn spawn_with_disabled_item(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    let menu = Menu::from_items(
        vec![
            MenuItem::new("start", "Start Game"),
            MenuItem::new("continue", "Continue").disabled(),
            MenuItem::new("quit", "Quit"),
        ],
        Vec2::new(100.0, 80.0),
        "arcade",
        24.0,
        30.0,
        true,
    )
    .with_disabled_color(Color::GRAY);

    commands.spawn(menu);
    } // GLUE

    fn spawn_with_text_cursor(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    let menu = Menu::new(
        &[("play", "Play"), ("quit", "Quit")],
        Vec2::new(200.0, 150.0),
        "arcade",
        32.0,
        40.0,
        true,
    )
    .with_cursor_text("→");

    commands.spawn(menu);
    } // GLUE

    fn replace_menu_items(mut commands: Commands, menu: Entity) { // GLUE
    use aberredengine::prelude::*;

    commands.entity(menu).insert(MenuRebuild::new(vec![
        MenuItem::new("again", "Play Again"),
        MenuItem::new("locked", "Locked").disabled(),
    ]));
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
            .with("play", MenuAction::RequestScene("level01".to_string()))
            .with("quit", MenuAction::RequestQuit);

        commands.spawn((menu, actions));
    }
}

// Animation finished event
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

// Tween finished event
mod tween_finished_event {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    fn on_move_tween_done(
        trigger: On<TweenFinished<WorldPosition>>,
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
