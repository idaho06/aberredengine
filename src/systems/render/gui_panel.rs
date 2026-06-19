use raylib::prelude::*;

use super::ScreenPanelBufferItem;

/// Draw one already-resolved screen-space GUI panel item (window background).
pub(super) fn draw_screen_panel_item(
    d: &mut impl RaylibDraw,
    item: &ScreenPanelBufferItem,
    textures: &crate::resources::texturestore::TextureStore,
) {
    let panel = &item.panel;
    if let Some(tex) = textures.get(&panel.tex_key) {
        let dest = Rectangle {
            x: item.pos.pos.x,
            y: item.pos.pos.y,
            width: item.size.x,
            height: item.size.y,
        };
        let n_patch_info = NPatchInfo {
            source: panel.source,
            left: panel.left,
            top: panel.top,
            right: panel.right,
            bottom: panel.bottom,
            layout: NPatchLayout::NPATCH_NINE_PATCH,
        };
        d.draw_texture_n_patch(
            tex,
            n_patch_info,
            dest,
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
    }
}
