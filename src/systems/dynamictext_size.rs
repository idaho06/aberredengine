//! DynamicText size caching system.
//!
//! Calculates and caches the bounding box size of [`DynamicText`] components
//! when they are added or modified. This avoids per-frame `MeasureTextEx` calls
//! in the render system.

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::prelude::*;
use raylib::ffi;
use raylib::math::Vector2;

use log::{debug, warn};

use crate::components::dynamictext::DynamicText;
use crate::resources::fontstore::FontStore;

/// Recalculates the cached size for any [`DynamicText`] that was added or changed.
///
/// Uses `bypass_change_detection` when updating the size field to avoid
/// re-triggering this system on the next frame.
pub fn dynamictext_size_system(
    mut query: Query<&mut DynamicText, Changed<DynamicText>>,
    fonts: NonSend<FontStore>,
) {
    for mut text in query.iter_mut() {
        debug!("Calculating size for DynamicText: '{}'", text.text);
        let Some(font) = fonts.get(&*text.font) else {
            warn!("Font '{}' not found in FontStore, text size will be zero", text.font);
            continue;
        };

        let text_c_string = std::ffi::CString::new(text.text.as_bytes())
            .expect("Failed to convert text content to CString");

        let measured = unsafe {
            ffi::MeasureTextEx(
                **font,
                text_c_string.as_ptr() as *const i8,
                text.font_size,
                1.0,
            )
        };

        // Convert ffi::Vector2 to math::Vector2
        let size = Vector2::new(measured.x, measured.y);
        text.bypass_change_detection().set_size(size);
    }
}
