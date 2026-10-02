// Testing your game
use aberredengine::bevy_ecs; // GLUE
use aberredengine::bevy_ecs::prelude::*; // GLUE
#[derive(Component)] // GLUE
struct Health(i32); // GLUE
fn remove_dead(mut commands: Commands, query: Query<(Entity, &Health)>) { // GLUE
    for (entity, health) in &query { // GLUE
        if health.0 <= 0 { // GLUE
            commands.entity(entity).despawn(); // GLUE
        } // GLUE
    } // GLUE
} // GLUE

#[cfg(test)]
mod tests {
    use super::{Health, remove_dead};
    use aberredengine::test_support::TestWorld;

    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn dead_entities_are_removed() {
        let mut tw = TestWorld::builder()
            .add_system(remove_dead)
            .build()
            .expect("test world");
        tw.tick_to_play(DT, 10);

        let enemy = tw.world.spawn(Health(0)).id();
        tw.tick(1, DT);

        assert!(tw.world.get_entity(enemy).is_err());
    }
}
