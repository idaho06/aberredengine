//! Window-resize detection.
//!
//! [`detect_window_resize`] diffs the logic world's [`WindowSize`] against
//! its previous-tick value and triggers [`WindowResizedEvent`] on a real,
//! positive-dimensioned change. `WindowSize` itself is mirrored from the
//! newest input sample's raw window dims directly in `logic_thread_main`'s
//! per-tick loop (`engine_app.rs`), before the sim schedule runs — this
//! system only reacts to that already-updated value, mirroring the
//! `Local<Option<T>>`-diff convention used by `send_render_mirrors`/
//! `apply_gameconfig_changes` on the render side.

use crate::events::window::WindowResizedEvent;
use crate::resources::windowsize::WindowSize;
use bevy_ecs::prelude::*;

/// Triggers [`WindowResizedEvent`] whenever [`WindowSize`] differs from its
/// value on this system's previous run, as long as both new dimensions are
/// `> 0`. The `Local` starting at `None` is treated as "no baseline
/// established yet" rather than "changed" -- the first run only records
/// `WindowSize`'s current value without firing, matching
/// [`WindowResizedEvent`]'s documented "not triggered on the first tick"
/// contract (unlike `send_render_mirrors`'s `Local<Option<T>>` fields on
/// the render side, where a first-frame fire is harmless; here it would
/// misreport the window's startup size as a resize).
pub fn detect_window_resize(
    window_size: Res<WindowSize>,
    mut last: Local<Option<WindowSize>>,
    mut commands: Commands,
) {
    let Some(prev) = *last else {
        *last = Some(*window_size);
        return;
    };
    if prev != *window_size {
        *last = Some(*window_size);
        if window_size.w > 0 && window_size.h > 0 {
            commands.trigger(WindowResizedEvent {
                w: window_size.w,
                h: window_size.h,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Resource, Default)]
    struct ResizeCount(u32);

    fn count_resizes(_event: On<WindowResizedEvent>, mut count: ResMut<ResizeCount>) {
        count.0 += 1;
    }

    /// Builds a world/schedule pair and runs the schedule once to consume
    /// `detect_window_resize`'s baseline-establishing first run, so every
    /// test can start from "a baseline is already recorded" instead of
    /// repeating that step itself.
    fn test_world_and_schedule(w: i32, h: i32) -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(WindowSize { w, h });
        world.insert_resource(ResizeCount::default());
        world.spawn(Observer::new(count_resizes));
        world.flush();
        let mut schedule = Schedule::default();
        schedule.add_systems(detect_window_resize);
        schedule.run(&mut world);
        (world, schedule)
    }

    fn set_window_size(world: &mut World, w: i32, h: i32) {
        let mut window_size = world.resource_mut::<WindowSize>();
        window_size.w = w;
        window_size.h = h;
    }

    #[test]
    fn first_run_at_startup_size_does_not_trigger() {
        let (world, _schedule) = test_world_and_schedule(800, 600);
        assert_eq!(world.resource::<ResizeCount>().0, 0);
    }

    #[test]
    fn real_change_triggers_once() {
        let (mut world, mut schedule) = test_world_and_schedule(800, 600);
        set_window_size(&mut world, 1024, 768);
        schedule.run(&mut world);
        assert_eq!(world.resource::<ResizeCount>().0, 1);
    }

    #[test]
    fn repeated_same_value_triggers_only_first_time() {
        let (mut world, mut schedule) = test_world_and_schedule(800, 600);
        set_window_size(&mut world, 1024, 768);
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(world.resource::<ResizeCount>().0, 1);
    }

    #[test]
    fn ignores_nonpositive_dimensions() {
        let (mut world, mut schedule) = test_world_and_schedule(800, 600);
        set_window_size(&mut world, 0, 768);
        schedule.run(&mut world);
        set_window_size(&mut world, 1024, -1);
        schedule.run(&mut world);
        assert_eq!(world.resource::<ResizeCount>().0, 0);
    }
}
