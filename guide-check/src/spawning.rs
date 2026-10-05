// Example 1: Sprite entity
mod example_1_sprite_entity {
    use aberredengine::prelude::*; // GLUE

    fn enter(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
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

    fn enter(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
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

    fn enter(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
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

    fn enter(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;
    use aberredengine::core::components::animation::{CmpOp, Condition};

    commands.spawn((
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

    fn map_position(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
        MapPosition::new(0.0, 0.0),
        Tween::position(Vec2::ZERO, Vec2::new(200.0, 120.0), 1.5)
            .with_easing(Easing::CubicOut)
            .with_loop_mode(LoopMode::PingPong),
    ));
    } // GLUE

    fn rotation(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
        Rotation::new(0.0),
        Tween::rotation(0.0, 360.0, 2.0),
    ));
    } // GLUE

    fn scale(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
        Scale::new(1.0, 1.0),
        Tween::scale(Vec2::ONE, Vec2::new(1.5, 0.75), 0.75)
            .with_backwards(),
    ));
    } // GLUE

    fn screen_position(mut commands: Commands) { // GLUE
    use aberredengine::prelude::*;

    commands.spawn((
        ScreenPosition::new(-200.0, 50.0),
        Tween::screen_position(Vec2::new(-200.0, 50.0), Vec2::new(20.0, 50.0), 0.4),
    ));
    } // GLUE
}

// Spawning context: observers and systems
mod spawning_context_observers_and_systems {
    use aberredengine::prelude::*; // GLUE

    fn enter(_: On<SceneEntered>, mut commands: Commands) {
        commands.spawn(( /* ... */ ));
    }

    fn my_update(mut commands: Commands) {
        commands.spawn(( /* ... */ ));
    }
}
