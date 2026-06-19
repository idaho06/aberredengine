use raylib::prelude::*;

use super::ScreenSpriteBufferItem;

/// Draw one already-resolved screen-space sprite item (UI layer).
pub(super) fn draw_screen_sprite_item(
    d: &mut impl RaylibDraw,
    item: &ScreenSpriteBufferItem,
    textures: &crate::resources::texturestore::TextureStore,
    debug: bool,
) {
    let sprite = &item.sprite;
    let pos = item.pos;
    if let Some(tex) = textures.get(&sprite.tex_key) {
        let mut src = Rectangle {
            x: sprite.offset.x,
            y: sprite.offset.y,
            width: sprite.width,
            height: sprite.height,
        };
        if sprite.flip_h {
            src.width = -src.width;
        }
        if sprite.flip_v {
            src.height = -src.height;
        }

        let dest = Rectangle {
            x: pos.pos.x,
            y: pos.pos.y,
            width: sprite.width,
            height: sprite.height,
        };

        let tint_color = item.maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
        d.draw_texture_pro(
            tex,
            src,
            dest,
            Vector2 {
                x: sprite.origin.x,
                y: sprite.origin.y,
            },
            0.0,
            tint_color,
        );
    }
    if debug {
        d.draw_rectangle_lines(
            pos.pos.x as i32 - sprite.origin.x as i32,
            pos.pos.y as i32 - sprite.origin.y as i32,
            sprite.width as i32,
            sprite.height as i32,
            Color::PURPLE,
        );
        d.draw_line(
            pos.pos.x as i32 - 6,
            pos.pos.y as i32 - 6,
            pos.pos.x as i32 + 6,
            pos.pos.y as i32 + 6,
            Color::PURPLE,
        );
        d.draw_line(
            pos.pos.x as i32 + 6,
            pos.pos.y as i32 - 6,
            pos.pos.x as i32 - 6,
            pos.pos.y as i32 + 6,
            Color::PURPLE,
        );
    }
}
