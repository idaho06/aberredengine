// Example 1: Sprite entity
mod example_1_sprite_entity {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        MapPosition::new(100.0, 200.0),
        Sprite::new("player", 32.0, 32.0).centered(),
        ZIndex(1.0),
        Group::new("player"),
    ));
    } // GLUE
}

// Example 2: Physics entity
mod example_2_physics_entity {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        MapPosition::new(100.0, 200.0),
        Sprite::new("player", 32.0, 32.0).centered(),
        ZIndex(1.0),
        Group::new("player"),
        RigidBody::with_physics(5.0, Some(300.0)),  // friction=5.0, max_speed=300
        BoxCollider::new(28.0, 30.0)
            .with_origin(Vec2::new(16.0, 16.0))
            .with_offset(Vec2::new(2.0, 2.0)),
        AccelerationControlled::symmetric(800.0),    // 800 units/s² in all directions
    ));
    } // GLUE
}

// Example 3: UI text with signal binding
mod example_3_ui_text_with_signal_binding {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        ScreenPosition::new(10.0, 10.0),
        DynamicText::new("0", "arcade", 16.0, Color::WHITE),
        SignalBinding::new("score").with_format("Score: {}"),
        ZIndex(100.0),
    ));
    } // GLUE
}

// Component constructor quick reference: animation controller rules
mod animation_controller_rules {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;
    use aberredengine::core::components::animation::{CmpOp, Condition};

    ctx.commands.spawn((
        Animation::new("player_idle"),
        AnimationController::new("player_idle")
            .with_rule(Condition::HasFlag { key: "dead".into() }, "player_dead")
            .with_rule(
                Condition::ScalarCmp { key: "speed".into(), op: CmpOp::Gt, value: 1.0 },
                "player_run",
            ),
        Signals::default(), // write "dead" / "speed" here from your game logic
    ));
    } // GLUE
}

// Tween components in Rust
mod tween_components_in_rust {
    use aberredengine::prelude::*; // GLUE

    fn map_position(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        MapPosition::new(0.0, 0.0),
        Tween::position(Vec2::ZERO, Vec2::new(200.0, 120.0), 1.5)
            .with_easing(Easing::CubicOut)
            .with_loop_mode(LoopMode::PingPong),
    ));
    } // GLUE

    fn rotation(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        Rotation::new(0.0),
        Tween::rotation(0.0, 360.0, 2.0),
    ));
    } // GLUE

    fn scale(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        Scale::new(1.0, 1.0),
        Tween::scale(Vec2::ONE, Vec2::new(1.5, 0.75), 0.75)
            .with_backwards(),
    ));
    } // GLUE

    fn screen_position(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
        ScreenPosition::new(-200.0, 50.0),
        Tween::screen_position(Vec2::new(-200.0, 50.0), Vec2::new(20.0, 50.0), 0.4),
    ));
    } // GLUE
}

// Spawning context: GameCtx vs. raw hooks
mod spawning_context_gamectx_vs_raw_hooks {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) {
        ctx.commands.spawn(( /* ... */ ));
    }

    fn my_enter_play(mut commands: Commands) {
        commands.spawn(( /* ... */ ));
    }
}

// 6.2 Triggering scene transitions
mod triggering_scene_transitions {
    use aberredengine::prelude::*; // GLUE
    fn player_reached_exit(_ctx: &mut GameCtx) -> bool { false } // GLUE

    fn update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
        if player_reached_exit(ctx) {
            ctx.world_signals.request_scene("level02");
        }
    }
}

// 6.3 Persistent entities
mod persistent_entities {
    use aberredengine::prelude::*; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::prelude::*;

    ctx.commands.spawn((
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

// 6.4 Group tracking across scenes
mod group_tracking_across_scenes {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    EngineBuilder::new()
        .track_group("enemies")
        .track_group("bricks")
        // …
    } // GLUE
}

// 6.5 Per-sim-tick scene updates
mod per_sim_tick_scene_updates {
    use aberredengine::prelude::*; // GLUE

    fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) {
        // dt = world_time.delta (the fixed sim period, 1.0 / hz, scaled by time_scale)
        // input = current keyboard state (just_pressed, active, just_released)
        // Use ctx to read/write ECS state once per sim tick
    }
}

// Your own components and resources
mod own_components_and_resources {
    use aberredengine::prelude::*; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::prelude::*;

    #[derive(Component)]
    struct Health(i32);

    #[derive(Resource, Default)]
    struct Score(u32);

    fn setup(mut commands: Commands) {
        commands.insert_resource(Score::default());
    }

    fn remove_dead(mut commands: Commands, mut score: ResMut<Score>, query: Query<(Entity, &Health)>) {
        for (entity, health) in &query {
            if health.0 <= 0 {
                commands.entity(entity).despawn();
                score.0 += 100;
            }
        }
    }

    EngineBuilder::new()
        .on_setup(setup)
        .add_system(remove_dead)
        // …
    } // GLUE
}
