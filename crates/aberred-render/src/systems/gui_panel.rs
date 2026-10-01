use raylib::prelude::*;

use super::math::{rect_to_raylib, shadow_color};
use super::render::{ScreenPanelBufferItem, ScreenProgressBarBufferItem};
use crate::resources::texturestore::TextureStore;
use aberred_core::components::shadow::Shadow;
use aberred_core::resources::guitheme::GuiNinePatch;

/// Draw one already-resolved screen-space GUI panel item (window background).
pub(super) fn draw_screen_panel_item(
    d: &mut impl RaylibDraw,
    item: &ScreenPanelBufferItem,
    textures: &TextureStore,
) {
    if let Some(shadow) = item.maybe_shadow {
        draw_nine_patch_tinted(
            d,
            &item.panel,
            shadow_offset_rect(item.dest, shadow),
            shadow_color(shadow),
            textures,
        );
    }
    draw_nine_patch(d, &item.panel, item.dest, textures);
}

/// Draw one screen-space progress bar: optional track nine-patch at full size,
/// then fill nine-patch at the precomputed proportional destination. The track
/// is always drawn before the fill — this ordering is guaranteed by the single
/// `ScreenDrawItem::ProgressBar` variant design (no sort ambiguity).
pub(super) fn draw_screen_progress_bar_item(
    d: &mut impl RaylibDraw,
    item: &ScreenProgressBarBufferItem,
    textures: &TextureStore,
) {
    if let Some(shadow) = item.maybe_shadow {
        let shadow_dest = shadow_offset_rect(item.track_dest, shadow);
        let shadow_patch = item.track.as_ref().unwrap_or(&item.fill);
        draw_nine_patch_tinted(d, shadow_patch, shadow_dest, shadow_color(shadow), textures);
    }
    if let Some(track) = &item.track {
        draw_nine_patch(d, track, item.track_dest, textures);
    }
    if item.fill_dest.width > 0.0 && item.fill_dest.height > 0.0 {
        draw_nine_patch(d, &item.fill, item.fill_dest, textures);
    }
}

fn draw_nine_patch(
    d: &mut impl RaylibDraw,
    patch: &GuiNinePatch,
    dest: Rectangle,
    textures: &TextureStore,
) {
    draw_nine_patch_tinted(d, patch, dest, Color::WHITE, textures);
}

fn shadow_offset_rect(rect: Rectangle, shadow: Shadow) -> Rectangle {
    Rectangle {
        x: rect.x + shadow.offset.x,
        y: rect.y + shadow.offset.y,
        ..rect
    }
}

fn draw_nine_patch_tinted(
    d: &mut impl RaylibDraw,
    patch: &GuiNinePatch,
    dest: Rectangle,
    color: Color,
    textures: &TextureStore,
) {
    if let Some(tex) = textures.get(&patch.tex_key) {
        d.draw_texture_n_patch(
            tex,
            NPatchInfo {
                source: rect_to_raylib(patch.source),
                left: patch.left,
                top: patch.top,
                right: patch.right,
                bottom: patch.bottom,
                layout: NPatchLayout::NPATCH_NINE_PATCH,
            },
            dest,
            Vector2::new(0.0, 0.0),
            0.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_offset_rect_moves_the_rect_by_the_offset_and_keeps_its_size() {
        let rect = Rectangle {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        let moved = shadow_offset_rect(rect, Shadow::new(2.0, -3.0, 0, 0, 0, 128));
        assert_eq!(
            (moved.x, moved.y, moved.width, moved.height),
            (12.0, 17.0, 30.0, 40.0)
        );
    }
}
