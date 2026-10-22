// Hybrid games (Rust + Lua)
use aberredengine::prelude::*;

// Rust-side marker for the swarm members a Lua script spawns.
#[derive(Component)]
struct Boid;

// Lua spawns with `:with_group("swarm")`; Rust adopts every such entity.
fn adopt_swarm(ev: On<Add, Group>, groups: Query<&Group>, mut commands: Commands) {
    let entity = ev.event_target();
    if groups.get(entity).is_ok_and(|group| group.name() == "swarm") {
        commands.entity(entity).insert(Boid);
    }
}

// Runs before Lua's on_scene_enter, which sees this value and may override it.
fn seed_tuning(_: On<SceneEntered>, mut signals: ResMut<WorldSignals>) {
    signals.set_scalar("swarm_aggression", 1.0);
}

// Lua tunes the swarm with `engine.set_scalar("swarm_aggression", 2.0)`.
fn swarm_steering(signals: Res<WorldSignals>, mut boids: Query<&mut RigidBody, With<Boid>>) {
    let max_speed = Some(120.0 * signals.get_scalar("swarm_aggression").unwrap_or(1.0));
    for mut body in &mut boids {
        if body.max_speed != max_speed {
            body.max_speed = max_speed;
        }
    }
}

fn hybrid_game() -> EngineBuilder {
    EngineBuilder::new()
        .with_lua("assets/scripts/main.lua")
        .add_scene("asteroids_level01")
        .initial_scene("asteroids_level01")
        .add_observer(adopt_swarm)
        .on_scene_enter("asteroids_level01", seed_tuning)
        .add_scene_system("asteroids_level01", swarm_steering)
}

#[cfg(test)]
mod tests {
    use super::{Boid, adopt_swarm};
    use aberredengine::prelude::*;
    use aberredengine::test_support::TestWorld;

    const DT: f32 = 1.0 / 60.0;

    /// Entities the script spawns into the `"swarm"` group become `Boid`s.
    #[test]
    fn lua_swarm_entities_are_adopted_by_rust() {
        let script = std::env::temp_dir().join(format!(
            "guide_check_hybrid_{}.lua",
            std::process::id()
        ));
        std::fs::write(
            &script,
            r#"
function on_scene_enter(scene)
    engine.spawn():with_group("swarm"):build()
    engine.spawn():with_group("swarm"):build()
end
"#,
        )
        .unwrap();
        let mut tw = TestWorld::builder()
            .with_lua(&script)
            .add_scene("asteroids_level01")
            .initial_scene("asteroids_level01")
            .add_observer(adopt_swarm)
            .build()
            .expect("test world");
        std::fs::remove_file(&script).ok();
        tw.tick_to_play(DT, 8);

        let boids = tw
            .world
            .query_filtered::<(), With<Boid>>()
            .iter(&tw.world)
            .count();
        assert_eq!(boids, 2);
    }
}
