use bevy_ecs::prelude::Query;
use raylib::prelude::*;

use crate::components::dynamictext::DynamicText;
use crate::components::screenposition::ScreenPosition;
use crate::components::tint::Tint;
use crate::resources::fontstore::FontStore;

/// Draw screen-space dynamic texts (UI layer).
pub(super) fn draw_screen_texts(
    d: &mut impl RaylibDraw,
    query: &Query<(&DynamicText, &ScreenPosition, Option<&Tint>)>,
    fonts: &FontStore,
    debug: bool,
) {
    for (text, pos, maybe_tint) in query.iter() {
        if let Some(font) = fonts.get(&text.font) {
            let final_color = maybe_tint
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
}
