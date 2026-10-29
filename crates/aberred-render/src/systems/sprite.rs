use raylib::prelude::*;

use super::geometry::draw_rotated_rect_lines;
use super::gui_panel::shadow_offset_rect;
use super::math::{rect_to_raylib, resolve_sprite_tint, shadow_color, vec2_to_raylib};
use super::postprocess::{
    active_entity_shader, set_entity_uniforms, set_standard_uniforms, set_uniform_value,
};
use super::render::{DrawEnv, ScreenSpriteBufferItem, SpriteBufferItem};
use crate::resources::shaderstore::ShaderStore;
use aberred_core::components::sprite::Sprite;
use aberred_core::systems::sprite_geometry::compute_sprite_geometry;

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
            let shadow_dest = shadow_offset_rect(dest, shadow);
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

/// Draw one already-resolved world-space sprite item: shadow pass, then the
/// sprite itself (through its entity shader when it has a valid one), then the
/// debug bounds box.
pub(super) fn draw_world_sprite_item(
    d: &mut impl RaylibDraw,
    item: &SpriteBufferItem,
    shader_store: &mut ShaderStore,
    env: &DrawEnv,
) {
    if let Some(tex) = env.textures.get(&item.sprite.tex_key) {
        let src = sprite_src_rect(&item.sprite);

        let geom = compute_sprite_geometry(
            &item.resolved_pos,
            &item.sprite,
            item.resolved_scale.as_ref(),
            item.resolved_rot.as_ref(),
        );
        let dest = rect_to_raylib(geom.dest);
        let origin_scaled = vec2_to_raylib(geom.origin);

        let tint_color = resolve_sprite_tint(item.maybe_tint);

        if let Some(shadow) = item.maybe_shadow {
            let shadow_dest = shadow_offset_rect(dest, shadow);
            d.draw_texture_pro(
                tex,
                src,
                shadow_dest,
                origin_scaled,
                geom.rotation,
                shadow_color(shadow),
            );
        }

        if let Some((entity_shader, entry)) =
            active_entity_shader(shader_store, item.maybe_shader.as_ref())
        {
            set_standard_uniforms(
                &mut entry.shader,
                &mut entry.locations,
                env.world_time,
                env.render_size,
                env.window_size,
                &dest,
            );
            set_entity_uniforms(
                &mut entry.shader,
                &mut entry.locations,
                item.entity,
                &item.resolved_pos,
                item.resolved_rot.as_ref(),
                item.resolved_scale.as_ref(),
                Vector2 {
                    x: item.sprite.width,
                    y: item.sprite.height,
                },
                item.velocity,
            );
            for (name, value) in entity_shader.uniforms.iter() {
                set_uniform_value(&mut entry.shader, &mut entry.locations, name, value);
            }
            let mut d_shader = d.begin_shader_mode(&mut entry.shader);
            d_shader.draw_texture_pro(tex, src, dest, origin_scaled, geom.rotation, tint_color);
        } else {
            d.draw_texture_pro(tex, src, dest, origin_scaled, geom.rotation, tint_color);
        }

        if env.show_sprite_bounds {
            draw_rotated_rect_lines(d, dest, origin_scaled, geom.rotation, Color::BLUE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aberred_core::math::Vec2;

    fn sprite(flip_h: bool, flip_v: bool) -> Sprite {
        Sprite::new("atlas", 16.0, 24.0)
            .with_offset(Vec2::new(32.0, 48.0))
            .with_origin(Vec2::new(8.0, 12.0))
            .with_flip(flip_h, flip_v)
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
