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
    let text = &item.text;
    let pos = item.pos;
    if let Some(font) = fonts.get(&text.font) {
        let final_color = item
            .maybe_tint
            .map(|t| t.multiply(text.color))
            .unwrap_or(text.color);
        d.draw_text_ex(font, &text.text, pos.pos, text.font_size, 1.0, final_color);
        if debug {
            d.draw_rectangle_lines(
                pos.pos.x as i32,
                pos.pos.y as i32,
                text.size().x as i32,
                text.size().y as i32,
                Color::ORANGE,
            );
        }
    }
}
