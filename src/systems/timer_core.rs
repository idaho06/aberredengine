use bevy_ecs::prelude::*;

use crate::components::timer::Timer;

pub(crate) trait TimerRunner<C> {
    fn on_fire(&mut self, entity: Entity, callback: &C);
}

pub(crate) fn run_timer_update<C, R>(
    delta: f32,
    query: &mut Query<(Entity, &mut Timer<C>)>,
    runner: &mut R,
) where
    C: Send + Sync + 'static,
    R: TimerRunner<C>,
{
    for (entity, mut timer) in query.iter_mut() {
        timer.elapsed += delta;
        if timer.elapsed >= timer.duration {
            runner.on_fire(entity, &timer.callback);
            timer.reset();
        }
    }
}
