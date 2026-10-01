use raylib::prelude::*;

use super::math::{resolve_sprite_tint, shadow_color};
use super::render::ScreenSpriteBufferItem;
use aberred_core::components::sprite::Sprite;

/// Source rect of a sprite's atlas cell for `draw_texture_pro`: a flip is a
/// negative width/height, which is how raylib mirrors the sampled region.
pub(super) fn sprite_src_rect(sprite: &Sprite) -> Rectangle {
    let sign = |flip: bool| if flip { -1.0 } else { 1.0 };
    Rectangle {
        x: sprite.offset.x,
        y: sprite.offset.y,
        width: sprite.width * sign(sprite.flip_h),
        height: sprite.height * sign(sprite.flip_v),
    }
}

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
        let src = sprite_src_rect(sprite);

        let dest = Rectangle {
            x: pos.pos.x,
            y: pos.pos.y,
            width: sprite.width,
            height: sprite.height,
        };
        let origin = Vector2 {
            x: sprite.origin.x,
            y: sprite.origin.y,
        };

        if let Some(shadow) = item.maybe_shadow {
            let shadow_dest = Rectangle {
                x: dest.x + shadow.offset.x,
                y: dest.y + shadow.offset.y,
                ..dest
            };
            d.draw_texture_pro(tex, src, shadow_dest, origin, 0.0, shadow_color(shadow));
        }

        let tint_color = resolve_sprite_tint(item.maybe_tint);
        d.draw_texture_pro(tex, src, dest, origin, 0.0, tint_color);
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

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::math::Vec2;

    fn sprite(flip_h: bool, flip_v: bool) -> Sprite {
        Sprite {
            tex_key: std::sync::Arc::from("atlas"),
            width: 16.0,
            height: 24.0,
            offset: Vec2::new(32.0, 48.0),
            origin: Vec2::new(8.0, 12.0),
            flip_h,
            flip_v,
        }
    }

    fn xywh(r: Rectangle) -> (f32, f32, f32, f32) {
        (r.x, r.y, r.width, r.height)
    }

    #[test]
    fn src_rect_is_the_atlas_cell_with_flips_as_negative_extents() {
        assert_eq!(
            xywh(sprite_src_rect(&sprite(false, false))),
            (32.0, 48.0, 16.0, 24.0)
        );
        assert_eq!(
            xywh(sprite_src_rect(&sprite(true, false))),
            (32.0, 48.0, -16.0, 24.0)
        );
        assert_eq!(
            xywh(sprite_src_rect(&sprite(false, true))),
            (32.0, 48.0, 16.0, -24.0)
        );
        assert_eq!(
            xywh(sprite_src_rect(&sprite(true, true))),
            (32.0, 48.0, -16.0, -24.0)
        );
    }
}
