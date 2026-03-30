use bevy_ecs::prelude::Query;
use raylib::prelude::*;

use crate::components::screenposition::ScreenPosition;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::resources::texturestore::TextureStore;

/// Draw screen-space sprites (UI layer).
pub(super) fn draw_screen_sprites(
    d: &mut impl RaylibDraw,
    query: &Query<(&Sprite, &ScreenPosition, Option<&Tint>)>,
    textures: &TextureStore,
    debug: bool,
) {
    for (sprite, pos, maybe_tint) in query.iter() {
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

            let tint_color = maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
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
}
