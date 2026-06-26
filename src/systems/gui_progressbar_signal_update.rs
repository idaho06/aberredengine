//! Keeps `GuiProgressBar.value` in sync with `WorldSignals` for signal-bound bars.
//!
//! Runs every frame before `render_system`. For each bar with `signal_binding`
//! set, reads the named key from `WorldSignals` (integer preferred over
//! scalar, matching `update_world_signals_binding_system`'s priority) and
//! writes it into `bar.value`, clamped to `[0, bar.max]`.

use bevy_ecs::prelude::*;

use crate::components::guiprogressbar::GuiProgressBar;
use crate::resources::worldsignals::WorldSignals;

pub fn gui_progressbar_signal_update_system(
    mut query: Query<&mut GuiProgressBar>,
    world_signals: Res<WorldSignals>,
) {
    for mut bar in &mut query {
        let Some(key) = &bar.signal_binding else { continue; };
        let value = world_signals
            .get_integer(key)
            .map(|i| i as f32)
            .or_else(|| world_signals.get_scalar(key));
        let Some(v) = value else { continue; };
        let clamped = v.clamp(0.0, bar.max);
        if (bar.value - clamped).abs() > f32::EPSILON {
            bar.value = clamped;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    use crate::components::guiprogressbar::GuiProgressBar;
    use crate::resources::worldsignals::WorldSignals;

    fn tick<M>(world: &mut World, system: impl IntoSystem<(), (), M>) {
        world.run_system_once(system).expect("system should run");
    }

    #[test]
    fn updates_value_from_integer_signal() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world
            .resource_mut::<WorldSignals>()
            .set_integer("hp", 40);
        world.spawn(GuiProgressBar::new(200.0, 16.0, 100.0, 100.0).with_signal_binding("hp"));

        tick(&mut world, gui_progressbar_signal_update_system);

        let bar = world.query::<&GuiProgressBar>().single(&world).unwrap();
        assert!((bar.value - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn updates_value_from_scalar_signal() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world
            .resource_mut::<WorldSignals>()
            .set_scalar("energy", 0.75);
        world.spawn(GuiProgressBar::new(200.0, 16.0, 1.0, 1.0).with_signal_binding("energy"));

        tick(&mut world, gui_progressbar_signal_update_system);

        let bar = world.query::<&GuiProgressBar>().single(&world).unwrap();
        assert!((bar.value - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn clamps_value_to_max() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world
            .resource_mut::<WorldSignals>()
            .set_integer("hp", 9999);
        world.spawn(GuiProgressBar::new(200.0, 16.0, 0.0, 100.0).with_signal_binding("hp"));

        tick(&mut world, gui_progressbar_signal_update_system);

        let bar = world.query::<&GuiProgressBar>().single(&world).unwrap();
        assert!((bar.value - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn no_update_when_key_missing() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world.spawn(GuiProgressBar::new(200.0, 16.0, 50.0, 100.0).with_signal_binding("missing"));

        tick(&mut world, gui_progressbar_signal_update_system);

        let bar = world.query::<&GuiProgressBar>().single(&world).unwrap();
        assert!((bar.value - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn no_update_when_signal_binding_absent() {
        let mut world = World::new();
        world.insert_resource(WorldSignals::default());
        world
            .resource_mut::<WorldSignals>()
            .set_integer("hp", 10);
        world.spawn(GuiProgressBar::new(200.0, 16.0, 50.0, 100.0)); // no signal binding

        tick(&mut world, gui_progressbar_signal_update_system);

        let bar = world.query::<&GuiProgressBar>().single(&world).unwrap();
        assert!((bar.value - 50.0).abs() < f32::EPSILON);
    }
}
