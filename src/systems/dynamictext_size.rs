//! DynamicText size caching system.
//!
//! Calculates and caches the bounding box size of [`DynamicText`] components
//! when they are added or modified. This avoids per-frame `MeasureTextEx` calls
//! in the render system.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::*;

use log::{debug, warn};

use crate::components::dynamictext::DynamicText;
use crate::resources::fontmetrics::{FontMetricsStore, FontMetricsWarnCache};

/// Recalculates the cached size for any [`DynamicText`] that was added or changed.
///
/// Uses `bypass_change_detection` when updating the size field to avoid
/// re-triggering this system on the next frame.
///
/// Measures text CPU-side via [`FontMetricsStore`] instead of the GL-bound
/// `FontStore`/`ffi::MeasureTextEx`, so this system has no GL-context
/// dependency.
pub fn dynamictext_size_system(
    mut query: Query<&mut DynamicText, Changed<DynamicText>>,
    metrics: Res<FontMetricsStore>,
    mut warn_cache: ResMut<FontMetricsWarnCache>,
) {
    for mut text in query.iter_mut() {
        debug!("Calculating size for DynamicText: '{}'", text.text);
        let Some(font_metrics) = metrics.0.get(&*text.font) else {
            if warn_cache.warn_once(&text.font) {
                warn!(
                    "Font '{}' not found in FontMetricsStore, text size will be zero",
                    text.font
                );
            }
            continue;
        };

        let size = font_metrics.measure_text(&text.text, text.font_size, 1.0);
        text.bypass_change_detection().set_size(size);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::RunSystemOnce;

    use crate::components::dynamictext::DynamicText;
    use crate::resources::fontmetrics::test_support::lowercase_alphabet_metrics;
    use raylib::prelude::Color;

    fn new_test_world() -> World {
        let mut world = World::new();

        let mut store = FontMetricsStore::default();
        store
            .0
            .insert("test_font".to_string(), lowercase_alphabet_metrics());
        world.insert_resource(store);
        world.insert_resource(FontMetricsWarnCache::default());
        world
    }

    #[test]
    fn writes_fixture_predicted_size() {
        let mut world = new_test_world();
        world.spawn(DynamicText::new("abc", "test_font", 20.0, Color::WHITE));

        world
            .run_system_once(dynamictext_size_system)
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
            .run_system_once(dynamictext_size_system)
            .expect("system should run");
        world
            .run_system_once(dynamictext_size_system)
            .expect("system should run");

        let mut query = world.query::<&DynamicText>();
        let text = query.single(&world).unwrap();
        assert_eq!(text.size().x, 0.0);
        assert_eq!(text.size().y, 0.0);
    }
}
