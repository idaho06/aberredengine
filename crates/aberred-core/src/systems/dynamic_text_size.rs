//! DynamicText size caching system.
//!
//! Calculates and caches the bounding box size of [`DynamicText`] components
//! when they are added or modified. This avoids per-frame `MeasureTextEx` calls
//! in the render system.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::*;

use log::debug;

use crate::components::dynamic_text::DynamicText;
use crate::resources::font_metrics::{FontMetricsStore, FontMetricsWarnCache};
use crate::resources::pending_assets::PendingAssets;

/// The [`DynamicText`]s added or changed since the system last ran.
type ChangedTexts<'w, 's> = Query<'w, 's, &'static mut DynamicText, Changed<DynamicText>>;

/// Every [`DynamicText`].
type AllTexts<'w, 's> = Query<'w, 's, &'static mut DynamicText>;

/// Recalculates the cached size for any [`DynamicText`] that was added or
/// changed, and for every [`DynamicText`] when [`FontMetricsStore`] changed (a
/// text measured with the default font while its own font was pending, or
/// whose font was replaced, takes its real size when the metrics land).
///
/// An empty, reserved or unknown font key measures with the default font's
/// metrics ([`FontMetricsStore::resolve`]); an unknown key that is not
/// pending warns once. The size stays zero until the resolved font's metrics
/// exist.
///
/// Uses `bypass_change_detection` when updating the size field to avoid
/// re-triggering this system on the next frame.
///
/// Measures text CPU-side via [`FontMetricsStore`] instead of the GL-bound
/// `FontStore`/`ffi::MeasureTextEx`, so this system has no GL-context
/// dependency.
pub fn dynamic_text_size_system(
    mut texts: ParamSet<(ChangedTexts, AllTexts)>,
    metrics: Res<FontMetricsStore>,
    pending: Res<PendingAssets>,
    mut warn_cache: ResMut<FontMetricsWarnCache>,
) {
    let mut measure = |mut text: Mut<DynamicText>| {
        debug!("Calculating size for DynamicText: '{}'", text.text);
        let Some(font_metrics) = metrics.resolve(&text.font, &pending, &mut warn_cache) else {
            return;
        };
        let size = font_metrics.measure_text(&text.text, text.font_size, 1.0);
        text.bypass_change_detection().set_size(size);
    };
    if metrics.is_changed() {
        texts.p1().iter_mut().for_each(&mut measure);
    } else {
        texts.p0().iter_mut().for_each(&mut measure);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    use crate::components::dynamic_text::DynamicText;
    use crate::math::{Color, Vec2};
    use crate::protocol::asset_kind::AssetKind;
    use crate::resources::font_metrics::FontMetrics;
    use crate::resources::font_metrics::test_support::lowercase_alphabet_metrics;
    use crate::resources::pending_assets::PendingAssets;

    fn new_test_world() -> World {
        let mut world = World::new();

        let mut store = FontMetricsStore::default();
        store
            .0
            .insert("test_font".to_string(), lowercase_alphabet_metrics());
        world.insert_resource(store);
        world.insert_resource(FontMetricsWarnCache::default());
        world.insert_resource(PendingAssets::default());
        world
    }

    fn insert_metrics(world: &mut World, key: &str, metrics: FontMetrics) {
        world
            .resource_mut::<FontMetricsStore>()
            .0
            .insert(key.to_owned(), metrics);
    }

    /// The fixture font with every glyph twice as wide.
    fn wide_alphabet_metrics() -> FontMetrics {
        let mut metrics = lowercase_alphabet_metrics();
        for glyph in metrics.glyphs.values_mut() {
            glyph.advance_x *= 2;
        }
        metrics
    }

    fn only_text_size(world: &mut World) -> Vec2 {
        world.query::<&DynamicText>().single(world).unwrap().size()
    }

    fn run_once(world: &mut World) {
        world
            .run_system_once(dynamic_text_size_system)
            .expect("system should run");
    }

    #[test]
    fn empty_and_default_keys_measure_with_the_default_font() {
        for key in ["", "default"] {
            let mut world = new_test_world();
            insert_metrics(&mut world, "default", lowercase_alphabet_metrics());
            world.spawn(DynamicText::new("abc", key, 20.0, Color::WHITE));

            run_once(&mut world);

            assert_eq!(only_text_size(&mut world), Vec2::new(32.0, 20.0), "{key:?}");
            assert!(
                world.resource_mut::<FontMetricsWarnCache>().warn_once(key),
                "{key:?} is reserved and never warns"
            );
        }
    }

    #[test]
    fn unknown_key_falls_back_to_the_default_font_and_warns_once() {
        let mut world = new_test_world();
        insert_metrics(&mut world, "default", lowercase_alphabet_metrics());
        world.spawn(DynamicText::new("abc", "typo", 20.0, Color::WHITE));

        run_once(&mut world);

        assert_eq!(only_text_size(&mut world), Vec2::new(32.0, 20.0));
        assert!(
            !world
                .resource_mut::<FontMetricsWarnCache>()
                .warn_once("typo"),
            "the fallback already warned for 'typo'"
        );
    }

    #[test]
    fn a_pending_font_falls_back_without_warning() {
        let mut world = new_test_world();
        insert_metrics(&mut world, "default", lowercase_alphabet_metrics());
        world
            .resource_mut::<PendingAssets>()
            .queue(AssetKind::Font, "later");
        world.spawn(DynamicText::new("abc", "later", 20.0, Color::WHITE));

        run_once(&mut world);

        assert_eq!(only_text_size(&mut world), Vec2::new(32.0, 20.0));
        assert!(
            world
                .resource_mut::<FontMetricsWarnCache>()
                .warn_once("later"),
            "a pending font is not a typo"
        );
    }

    #[test]
    fn text_is_remeasured_when_its_pending_font_arrives() {
        let mut world = new_test_world();
        insert_metrics(&mut world, "default", lowercase_alphabet_metrics());
        world
            .resource_mut::<PendingAssets>()
            .queue(AssetKind::Font, "wide");
        world.spawn(DynamicText::new("abc", "wide", 20.0, Color::WHITE));
        let mut schedule = Schedule::default();
        schedule.add_systems(dynamic_text_size_system);

        schedule.run(&mut world);
        assert_eq!(only_text_size(&mut world).x, 32.0, "default font stand-in");

        insert_metrics(&mut world, "wide", wide_alphabet_metrics());
        world
            .resource_mut::<PendingAssets>()
            .settle(AssetKind::Font, "wide");
        schedule.run(&mut world);

        // "abc" @ 20 per glyph + 2 spacings of 1.0.
        assert_eq!(only_text_size(&mut world).x, 62.0);
    }

    #[test]
    fn a_loaded_font_is_measured_with_its_own_metrics() {
        let mut world = new_test_world();
        insert_metrics(&mut world, "default", wide_alphabet_metrics());
        world.spawn(DynamicText::new("abc", "test_font", 20.0, Color::WHITE));

        run_once(&mut world);

        assert_eq!(only_text_size(&mut world).x, 32.0);
    }

    #[test]
    fn writes_fixture_predicted_size() {
        let mut world = new_test_world();
        world.spawn(DynamicText::new("abc", "test_font", 20.0, Color::WHITE));

        world
            .run_system_once(dynamic_text_size_system)
            .expect("system should run");

        let mut query = world.query::<&DynamicText>();
        let text = query.single(&world).unwrap();
        // "abc" @ scale 1.0, spacing 1.0: 3*10*1.0 + (3-1)*1.0 = 32.0
        assert_eq!(text.size().x, 32.0);
        assert_eq!(text.size().y, 20.0);
    }

    #[test]
    fn missing_font_key_leaves_size_zero_and_does_not_panic() {
        let mut world = new_test_world();
        world.spawn(DynamicText::new("abc", "missing_font", 20.0, Color::WHITE));

        // Run twice to exercise the warn-once path without panicking.
        world
            .run_system_once(dynamic_text_size_system)
            .expect("system should run");
        world
            .run_system_once(dynamic_text_size_system)
            .expect("system should run");

        let mut query = world.query::<&DynamicText>();
        let text = query.single(&world).unwrap();
        assert_eq!(text.size().x, 0.0);
        assert_eq!(text.size().y, 0.0);
    }
}
