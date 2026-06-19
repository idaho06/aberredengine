use raylib::prelude::*;

use super::ScreenTextBufferItem;
use crate::resources::fontstore::FontStore;

/// Draw one already-resolved screen-space text item (UI layer).
pub(super) fn draw_screen_text_item(
    d: &mut impl RaylibDraw,
    item: &ScreenTextBufferItem,
    fonts: &FontStore,
    debug: bool,
) {
    let pos = item.pos;
    if let Some(font) = fonts.get(&item.font) {
        let final_color = item
            .maybe_tint
            .map(|t| t.multiply(item.color))
            .unwrap_or(item.color);
        d.draw_text_ex(font, &item.text, pos.pos, item.font_size, 1.0, final_color);
        if debug {
            d.draw_rectangle_lines(
                pos.pos.x as i32,
                pos.pos.y as i32,
                item.size.x as i32,
                item.size.y as i32,
                Color::ORANGE,
            );
        }
    }
}
