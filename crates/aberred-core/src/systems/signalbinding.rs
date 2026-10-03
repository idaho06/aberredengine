//! Signal binding system for reactive UI updates.
//!
//! This module provides the system that synchronizes [`DynamicText`]
//! components with signal values based on their [`SignalBinding`].

use arrayvec::ArrayString;
use std::fmt::Write as _;

/// Capacity for a formatted number. `f32`'s `Display` never uses exponent notation, so its
/// widest output is `-f32::MAX` at 40 chars (`i32::MIN` is 11). A buffer that is too small
/// makes `write!` fail part-way and the text shows a truncated number.
const NUM_BUF_LEN: usize = 48;

/// Stack-allocated string for signal-to-text conversion.
/// Uses a [`NUM_BUF_LEN`]-byte ArrayString for numeric types (i32 / f32), borrowed &str for others.
enum SignalStr<'a> {
    Stack(ArrayString<NUM_BUF_LEN>),
    Borrowed(&'a str),
}

impl<'a> SignalStr<'a> {
    fn as_str(&self) -> &str {
        match self {
            SignalStr::Stack(s) => s.as_str(),
            SignalStr::Borrowed(s) => s,
        }
    }
}

use crate::components::dynamictext::DynamicText;
use crate::components::signalbinding::{SignalBinding, SignalSource};
use crate::components::signals::Signals;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::*;

/// Updates [`DynamicText`] content based on signal bindings.
///
/// This system queries all entities with both `DynamicText` and `SignalBinding` components,
/// reads the corresponding signal value (from either `WorldSignals` or an entity's `Signals`),
/// and updates the text content accordingly.
///
/// Supported signal types:
/// - **Integer** - Displayed as-is (e.g., `"42"`)
/// - **Scalar** - Displayed as a floating-point number (e.g., `"3.14"`)
/// - **String** - Displayed as-is
/// - **Flag** - Displayed as `"true"` if set
///
/// If a format string is specified in the binding, the value replaces the `{}` placeholder.
///
/// Uses `bypass_change_detection` to avoid marking `DynamicText` as changed every frame.
/// Change detection is only triggered when content actually differs.
pub fn update_world_signals_binding_system(
    mut query: Query<(&mut DynamicText, &SignalBinding)>,
    world_signals: Res<WorldSignals>,
    signals_query: Query<&Signals>,
) {
    crate::tracy::tracy_span!("update_world_signals_binding");
    for (mut dynamic_text, signal_binding) in query.iter_mut() {
        let value_opt = match &signal_binding.source {
            SignalSource::World => {
                get_world_signal_as_str(&world_signals, &signal_binding.signal_key)
            }
            SignalSource::Entity(entity) => signals_query
                .get(*entity)
                .ok()
                .and_then(|signals| get_entity_signal_as_str(signals, &signal_binding.signal_key)),
        };

        if let Some(value) = value_opt {
            let new_text: std::borrow::Cow<str> = match &signal_binding.format {
                Some(fmt) => std::borrow::Cow::Owned(fmt.replace("{}", value.as_str())),
                None => std::borrow::Cow::Borrowed(value.as_str()),
            };
            // Bypass automatic change detection; manually mark as changed only if content differs
            let changed = dynamic_text
                .bypass_change_detection()
                .set_text(new_text.as_ref());
            if changed {
                dynamic_text.set_changed();
            }
        }
    }
}

/// Converts a signal value from [`WorldSignals`] to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag.
/// Returns `None` if the signal key is not found.
fn get_world_signal_as_str<'a>(
    world_signals: &'a WorldSignals,
    signal_key: &str,
) -> Option<SignalStr<'a>> {
    if let Some(v) = world_signals.get_integer(signal_key) {
        let mut buf = ArrayString::<NUM_BUF_LEN>::new();
        let _ = write!(buf, "{}", v);
        return Some(SignalStr::Stack(buf));
    }
    if let Some(v) = world_signals.get_scalar(signal_key) {
        let mut buf = ArrayString::<NUM_BUF_LEN>::new();
        let _ = write!(buf, "{}", v);
        return Some(SignalStr::Stack(buf));
    }
    if let Some(s) = world_signals.get_string(signal_key) {
        return Some(SignalStr::Borrowed(s));
    }
    if world_signals.has_flag(signal_key) {
        return Some(SignalStr::Borrowed("true"));
    }
    None
}

/// Converts a signal value from an entity's [`Signals`] component to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag.
/// Returns `None` if the signal key is not found.
fn get_entity_signal_as_str<'a>(signals: &'a Signals, signal_key: &str) -> Option<SignalStr<'a>> {
    if let Some(v) = signals.get_integer(signal_key) {
        let mut buf = ArrayString::<NUM_BUF_LEN>::new();
        let _ = write!(buf, "{}", v);
        return Some(SignalStr::Stack(buf));
    }
    if let Some(v) = signals.get_scalar(signal_key) {
        let mut buf = ArrayString::<NUM_BUF_LEN>::new();
        let _ = write!(buf, "{}", v);
        return Some(SignalStr::Stack(buf));
    }
    if let Some(s) = signals.get_string(signal_key) {
        return Some(SignalStr::Borrowed(s.as_str()));
    }
    if signals.has_flag(signal_key) {
        return Some(SignalStr::Borrowed("true"));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Color;
    use bevy_ecs::system::RunSystemOnce;

    fn text_entity(world: &mut World, binding: SignalBinding) -> Entity {
        world
            .spawn((
                DynamicText::new("placeholder", "font", 12.0, Color::WHITE),
                binding,
            ))
            .id()
    }

    fn run(world: &mut World) {
        world
            .run_system_once(update_world_signals_binding_system)
            .unwrap();
    }

    fn text(world: &World, e: Entity) -> String {
        world.get::<DynamicText>(e).unwrap().text.to_string()
    }

    fn world_with(signals: WorldSignals) -> World {
        let mut world = World::new();
        world.insert_resource(signals);
        world
    }

    #[test]
    fn displays_each_world_signal_type() {
        let mut ws = WorldSignals::default();
        ws.set_integer("lives", 3);
        ws.set_scalar("speed", 2.5);
        ws.set_string("name", "bob");
        ws.set_flag("paused");
        let mut world = world_with(ws);
        let e = ["lives", "speed", "name", "paused"]
            .map(|k| text_entity(&mut world, SignalBinding::new(k)));
        run(&mut world);
        assert_eq!(e.map(|e| text(&world, e)), ["3", "2.5", "bob", "true"]);
    }

    #[test]
    fn same_key_prefers_integer_then_scalar_then_string_then_flag() {
        let mut ws = WorldSignals::default();
        ws.set_integer("k", 1);
        ws.set_scalar("k", 2.5);
        ws.set_string("k", "s");
        ws.set_flag("k");
        let mut world = world_with(ws);
        let e = text_entity(&mut world, SignalBinding::new("k"));
        run(&mut world);
        assert_eq!(text(&world, e), "1");

        world.resource_mut::<WorldSignals>().remove_integer("k");
        run(&mut world);
        assert_eq!(text(&world, e), "2.5");
        world.resource_mut::<WorldSignals>().remove_scalar("k");
        run(&mut world);
        assert_eq!(text(&world, e), "s");
        world.resource_mut::<WorldSignals>().remove_string("k");
        run(&mut world);
        assert_eq!(text(&world, e), "true");
    }

    #[test]
    fn format_replaces_every_placeholder() {
        let mut ws = WorldSignals::default();
        ws.set_integer("score", 42);
        let mut world = world_with(ws);
        let e = text_entity(
            &mut world,
            SignalBinding::new("score").with_format("Score: {} ({})"),
        );
        run(&mut world);
        assert_eq!(text(&world, e), "Score: 42 (42)");
    }

    #[test]
    fn missing_key_leaves_text_unchanged() {
        let mut world = world_with(WorldSignals::default());
        let e = text_entity(
            &mut world,
            SignalBinding::new("absent").with_format("X: {}"),
        );
        run(&mut world);
        assert_eq!(text(&world, e), "placeholder");
    }

    #[test]
    fn entity_source_reads_that_entitys_signals_only() {
        let mut ws = WorldSignals::default();
        ws.set_integer("hp", 99);
        let mut world = world_with(ws);
        let mut signals = Signals::default();
        signals.set_integer("hp", 7);
        let owner = world.spawn(signals).id();
        let no_signals = world.spawn_empty().id();
        let despawned = world.spawn(Signals::default()).id();
        world.despawn(despawned);

        let bound = text_entity(
            &mut world,
            SignalBinding::new("hp").with_source_entity(owner),
        );
        let bare = text_entity(
            &mut world,
            SignalBinding::new("hp").with_source_entity(no_signals),
        );
        let dead = text_entity(
            &mut world,
            SignalBinding::new("hp").with_source_entity(despawned),
        );
        run(&mut world);
        assert_eq!(text(&world, bound), "7", "entity value, not the world's 99");
        assert_eq!(text(&world, bare), "placeholder");
        assert_eq!(text(&world, dead), "placeholder");
    }

    #[test]
    fn marks_text_changed_only_when_content_differs() {
        #[derive(Resource, Default)]
        struct ChangedCount(usize);
        fn count_changed(q: Query<(), Changed<DynamicText>>, mut n: ResMut<ChangedCount>) {
            n.0 += q.iter().count();
        }

        let mut ws = WorldSignals::default();
        ws.set_integer("score", 1);
        let mut world = world_with(ws);
        world.init_resource::<ChangedCount>();
        text_entity(&mut world, SignalBinding::new("score"));
        let mut schedule = Schedule::default();
        schedule.add_systems((update_world_signals_binding_system, count_changed).chain());

        schedule.run(&mut world); // spawn counts as changed; text also changes to "1"
        let after_first = world.resource::<ChangedCount>().0;
        schedule.run(&mut world); // same value: must not re-mark
        assert_eq!(world.resource::<ChangedCount>().0, after_first);
        world.resource_mut::<WorldSignals>().set_integer("score", 2);
        schedule.run(&mut world);
        assert_eq!(world.resource::<ChangedCount>().0, after_first + 1);
    }

    #[test]
    fn widest_numbers_are_displayed_in_full() {
        let mut ws = WorldSignals::default();
        ws.set_scalar("big", 1e35);
        ws.set_scalar("most_negative", -f32::MAX);
        ws.set_integer("min", i32::MIN);
        let mut world = world_with(ws);
        let mut entity_signals = Signals::default();
        entity_signals.set_scalar("big", -f32::MAX);
        let owner = world.spawn(entity_signals).id();

        let big = text_entity(&mut world, SignalBinding::new("big"));
        let neg = text_entity(&mut world, SignalBinding::new("most_negative"));
        let min = text_entity(&mut world, SignalBinding::new("min"));
        let from_entity = text_entity(
            &mut world,
            SignalBinding::new("big").with_source_entity(owner),
        );
        run(&mut world);

        assert_eq!(text(&world, big), 1e35_f32.to_string());
        assert_eq!(text(&world, neg), (-f32::MAX).to_string());
        assert_eq!(text(&world, min), i32::MIN.to_string());
        assert_eq!(text(&world, from_entity), (-f32::MAX).to_string());
    }
}
