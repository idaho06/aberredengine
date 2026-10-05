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

// Define your own `SystemParam` bundle
mod define_your_own_systemparam_bundle {
    use aberredengine::bevy_ecs::system::SystemParam;
    use aberredengine::core::resources::gamestate::{GameStates, NextGameState};
    use aberredengine::prelude::*;

    /// What this game's gameplay systems reach for, as one parameter.
    #[derive(SystemParam)]
    pub struct Game<'w, 's> {
        pub commands: Commands<'w, 's>,
        pub signals: ResMut<'w, WorldSignals>,
        pub time: Res<'w, WorldTime>,
        pub rng: ResMut<'w, SimRng>,
        pub assets: AssetLoader<'w>,
        next_state: ResMut<'w, NextGameState>,
    }

    impl Game<'_, '_> {
        pub fn play_sfx(&mut self, id: &str) {
            self.assets.audio().write(AudioCmd::PlayFx { id: id.into() });
        }

        pub fn quit(&mut self) {
            self.next_state.set(GameStates::Quitting);
        }
    }

    #[derive(Component)]
    struct Enemy;

    fn enemy_tick(mut game: Game, enemies: Query<(Entity, &MapPosition), With<Enemy>>) {
        for (entity, pos) in &enemies {
            if pos.pos.y > 600.0 {
                game.commands.entity(entity).despawn();
                game.play_sfx("splash");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::define_your_own_systemparam_bundle::Game;
    use aberredengine::bevy_ecs::message::Messages;
    use aberredengine::bevy_ecs::system::{RunSystemOnce, SystemParam};
    use aberredengine::core::resources::gamestate::{GameStates, NextGameState, NextGameStates};
    use aberredengine::prelude::*;
    use aberredengine::test_support::TestWorld;

    #[test]
    fn game_bundle_runs_in_the_logic_world() {
        let mut tw = TestWorld::builder().build().expect("test world");

        tw.world
            .run_system_once(|mut game: Game| {
                game.signals.set_flag("bundle_ran");
                game.play_sfx("jump");
                game.quit();
            })
            .expect("every param of the bundle resolves");

        assert!(tw.world.resource::<WorldSignals>().has_flag("bundle_ran"));
        let queued: Vec<AudioCmd> = tw.world.resource_mut::<Messages<AudioCmd>>().drain().collect();
        assert!(queued.iter().any(|c| matches!(c, AudioCmd::PlayFx { id } if id == "jump")));
        assert!(matches!(
            tw.world.resource::<NextGameState>().get(),
            NextGameStates::Pending(GameStates::Quitting)
        ));
    }

    #[test]
    #[should_panic(expected = "B0002")]
    fn a_sibling_param_that_overlaps_the_bundle_panics() {
        let _ = World::new().run_system_once(|_: Game, _: Res<WorldSignals>| {});
    }

    #[test]
    #[should_panic(expected = "B0002")]
    fn the_bundles_asset_loader_conflicts_with_an_audio_writer() {
        let _ = World::new().run_system_once(|_: Game, _: MessageWriter<AudioCmd>| {});
    }

    #[derive(SystemParam)]
    struct Movers<'w, 's> {
        positions: Query<'w, 's, &'static mut MapPosition>,
    }

    #[test]
    #[should_panic(expected = "B0001")]
    fn a_sibling_query_that_overlaps_a_bundled_query_panics() {
        let _ = World::new().run_system_once(|_: Movers, _: Query<&MapPosition>| {});
    }
}
