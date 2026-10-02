// Example 1: Sprite entity
mod example_1_sprite_entity {
    use aberredengine::core::systems::GameCtx; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::components::sprite::Sprite;
    use aberredengine::core::components::zindex::ZIndex;
    use aberredengine::core::components::group::Group;
    use aberredengine::core::math::Vec2;
    use std::sync::Arc;

    ctx.commands.spawn((
        MapPosition::new(100.0, 200.0),
        Sprite {
            tex_key: Arc::from("player"),
            width: 32.0,
            height: 32.0,
            offset: Vec2::ZERO,
            origin: Vec2::new(16.0, 16.0), // center pivot
            flip_h: false,
            flip_v: false,
        },
        ZIndex(1.0),
        Group::new("player"),
    ));
    } // GLUE
}

// Example 2: Physics entity
mod example_2_physics_entity {
    use aberredengine::core::components::group::Group; // GLUE
    use aberredengine::core::components::mapposition::MapPosition; // GLUE
    use aberredengine::core::components::sprite::Sprite; // GLUE
    use aberredengine::core::components::zindex::ZIndex; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE
    use std::sync::Arc; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::rigidbody::RigidBody;
    use aberredengine::core::components::boxcollider::BoxCollider;
    use aberredengine::core::components::inputcontrolled::AccelerationControlled;
    use aberredengine::core::math::Vec2;

    ctx.commands.spawn((
        MapPosition::new(100.0, 200.0),
        Sprite {
            tex_key: Arc::from("player"),
            width: 32.0,
            height: 32.0,
            offset: Vec2::ZERO,
            origin: Vec2::new(16.0, 16.0),
            flip_h: false,
            flip_v: false,
        },
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
    use aberredengine::core::components::zindex::ZIndex; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::screenposition::ScreenPosition;
    use aberredengine::core::components::dynamictext::DynamicText;
    use aberredengine::core::components::signalbinding::SignalBinding;
    use aberredengine::core::math::Color;

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
    use aberredengine::core::systems::GameCtx; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::animation::{Animation, AnimationController, CmpOp, Condition};
    use aberredengine::core::components::signals::Signals;

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
    use aberredengine::core::systems::GameCtx; // GLUE

    fn map_position(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::components::tween::{Easing, LoopMode, Tween};
    use aberredengine::core::math::Vec2;

    ctx.commands.spawn((
        MapPosition::new(0.0, 0.0),
        Tween::new(
            MapPosition::from_vec(Vec2::new(0.0, 0.0)),
            MapPosition::from_vec(Vec2::new(200.0, 120.0)),
            1.5,
        )
        .with_easing(Easing::CubicOut)
        .with_loop_mode(LoopMode::PingPong),
    ));
    } // GLUE

    fn rotation(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::rotation::Rotation;
    use aberredengine::core::components::tween::Tween;

    ctx.commands.spawn((
        Rotation { degrees: 0.0 },
        Tween::new(
            Rotation { degrees: 0.0 },
            Rotation { degrees: 360.0 },
            2.0,
        ),
    ));
    } // GLUE

    fn scale(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::scale::Scale;
    use aberredengine::core::components::tween::Tween;

    ctx.commands.spawn((
        Scale::new(1.0, 1.0),
        Tween::new(
            Scale::new(1.0, 1.0),
            Scale::new(1.5, 0.75),
            0.75,
        )
        .with_backwards(),
    ));
    } // GLUE

    fn screen_position(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::components::screenposition::ScreenPosition;
    use aberredengine::core::components::tween::Tween;

    ctx.commands.spawn((
        ScreenPosition::new(-200.0, 50.0),
        Tween::new(
            ScreenPosition::new(-200.0, 50.0),
            ScreenPosition::new(20.0, 50.0),
            0.4,
        ),
    ));
    } // GLUE
}

// Spawning context: GameCtx vs. raw hooks
mod spawning_context_gamectx_vs_raw_hooks {
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE

    fn enter(ctx: &mut GameCtx) {
        ctx.commands.spawn(( /* ... */ ));
    }

    fn my_enter_play(mut commands: Commands) {
        commands.spawn(( /* ... */ ));
    }
}

// 6.2 Triggering scene transitions
mod triggering_scene_transitions {
    use aberredengine::core::resources::input::InputState; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE
    fn player_reached_exit(_ctx: &mut GameCtx) -> bool { false } // GLUE

    use aberredengine::core::resources::signal_keys as sk;

    fn update(ctx: &mut GameCtx, _dt: f32, _input: &InputState) {
        if player_reached_exit(ctx) {
            ctx.world_signals.set_string(sk::SCENE, "level02".to_string());
            ctx.world_signals.set_flag(sk::SWITCH_SCENE);
        }
    }
}

// 6.3 Persistent entities
mod persistent_entities {
    use aberredengine::bevy_ecs::prelude::*; // GLUE
    use aberredengine::core::components::dynamictext::DynamicText; // GLUE
    use aberredengine::core::components::persistent::Persistent; // GLUE
    use aberredengine::core::components::screenposition::ScreenPosition; // GLUE
    use aberredengine::core::components::signalbinding::SignalBinding; // GLUE
    use aberredengine::core::components::zindex::ZIndex; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE

    fn enter(ctx: &mut GameCtx) { // GLUE
    use aberredengine::core::math::Color;

    ctx.commands.spawn((
        ScreenPosition::new(10.0, 10.0),
        DynamicText::new("0", "arcade", 16.0, Color::WHITE),
        SignalBinding::new("score").with_format("Score: {}"),
        ZIndex(100.0),
        Persistent,  // survives scene switches
    ));
    } // GLUE

    use aberredengine::core::components::persistent::CleanableEntity;

    fn my_cleanup(query: Query<Entity, CleanableEntity>, mut commands: Commands) {
        for entity in &query {
            commands.entity(entity).despawn();
        }
    }
}

// 6.4 Group tracking across scenes
mod group_tracking_across_scenes {
    use aberredengine::engine_app::EngineBuilder; // GLUE

    fn register() -> EngineBuilder { // GLUE
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::resources::group::TrackedGroups;

    fn track_groups(mut tracked: ResMut<TrackedGroups>) {
        for name in ["enemies", "bricks"] {
            if !tracked.has_group(name) {
                tracked.add_group(name);
            }
        }
    }

    EngineBuilder::new()
        .add_system(track_groups)
        // …
    } // GLUE
}

// 6.5 Per-sim-tick scene updates
mod per_sim_tick_scene_updates {
    use aberredengine::core::resources::input::InputState; // GLUE
    use aberredengine::core::systems::GameCtx; // GLUE

    fn update(ctx: &mut GameCtx, dt: f32, input: &InputState) {
        // dt = world_time.delta (the fixed sim period, 1.0 / hz, scaled by time_scale)
        // input = current keyboard state (just_pressed, active, just_released)
        // Use ctx to read/write ECS state once per sim tick
    }
}
