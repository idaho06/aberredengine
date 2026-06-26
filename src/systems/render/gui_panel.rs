use raylib::prelude::*;

use super::{ScreenPanelBufferItem, ScreenProgressBarBufferItem};
use crate::resources::guitheme::GuiNinePatch;
use crate::resources::texturestore::TextureStore;

/// Draw one already-resolved screen-space GUI panel item (window background).
pub(super) fn draw_screen_panel_item(
    d: &mut impl RaylibDraw,
    item: &ScreenPanelBufferItem,
    textures: &TextureStore,
) {
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
    if let Some(tex) = textures.get(&patch.tex_key) {
        d.draw_texture_n_patch(
            tex,
            NPatchInfo {
                source: patch.source,
                left: patch.left,
                top: patch.top,
                right: patch.right,
                bottom: patch.bottom,
                layout: NPatchLayout::NPATCH_NINE_PATCH,
            },
            dest,
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
    }
}
