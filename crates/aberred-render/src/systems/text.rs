use raylib::prelude::*;

use super::math::{resolve_text_tint, shadow_color, vec2_to_raylib};
use super::postprocess::{set_entity_uniforms, set_standard_uniforms, set_uniform_value};
use super::render::{DrawEnv, ScreenTextBufferItem, TextBufferItem};
use crate::resources::fontstore::FontStore;
use crate::resources::shaderstore::ShaderStore;

/// Draw one already-resolved world-space text item: shadow pass, then the text
/// itself (through its entity shader when it has a valid one), then the debug
/// bounds box.
pub(super) fn draw_world_text_item(
    d: &mut impl RaylibDraw,
    item: &TextBufferItem,
    shader_store: &mut ShaderStore,
    env: &DrawEnv,
) {
    if let Some((font, spacing)) = env.fonts.get_or_default(&item.text.font) {
        let final_color = resolve_text_tint(item.maybe_tint, item.text.color);
        let draw_pos = vec2_to_raylib(item.resolved_pos.pos);

        if let Some(shadow) = item.maybe_shadow {
            let shadow_pos = Vector2 {
                x: item.resolved_pos.pos.x + shadow.offset.x,
                y: item.resolved_pos.pos.y + shadow.offset.y,
            };
            d.draw_text_ex(
                font,
                &item.text.text,
                shadow_pos,
                item.text.font_size,
                spacing,
                shadow_color(shadow),
            );
        }

        let active = match &item.maybe_shader {
            Some(entity_shader) => shader_store
                .entity_shader_mut(entity_shader)
                .map(|entry| (entity_shader, entry)),
            None => None,
        };
        if let Some((entity_shader, entry)) = active {
            let dest = Rectangle {
                x: item.resolved_pos.pos.x,
                y: item.resolved_pos.pos.y,
                width: item.text_size.x,
                height: item.text_size.y,
            };
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
                None,
                None,
                item.text_size,
                item.velocity,
            );
            for (name, value) in entity_shader.uniforms.iter() {
                set_uniform_value(&mut entry.shader, &mut entry.locations, name, value);
            }
            let mut d_shader = d.begin_shader_mode(&mut entry.shader);
            d_shader.draw_text_ex(
                font,
                &item.text.text,
                draw_pos,
                item.text.font_size,
                spacing,
                final_color,
            );
        } else {
            d.draw_text_ex(
                font,
                &item.text.text,
                draw_pos,
                item.text.font_size,
                spacing,
                final_color,
            );
        }

        if env.show_text_bounds {
            d.draw_rectangle_lines(
                item.resolved_pos.pos.x as i32,
                item.resolved_pos.pos.y as i32,
                item.text_size.x as i32,
                item.text_size.y as i32,
                Color::ORANGE,
            );
        }
    }
}

/// Draw one already-resolved screen-space text item (UI layer).
pub(super) fn draw_screen_text_item(
    d: &mut impl RaylibDraw,
    item: &ScreenTextBufferItem,
    fonts: &FontStore,
    debug: bool,
) {
    let pos = item.pos;
    if let Some((font, spacing)) = fonts.get_or_default(&item.font) {
        let final_color = resolve_text_tint(item.maybe_tint, item.color);
        if let Some(shadow) = item.maybe_shadow {
            let shadow_pos = Vector2 {
                x: pos.pos.x + shadow.offset.x,
                y: pos.pos.y + shadow.offset.y,
            };
            d.draw_text_ex(
                font,
                &item.text,
                shadow_pos,
                item.font_size,
                spacing,
                shadow_color(shadow),
            );
        }
        d.draw_text_ex(
            font,
            &item.text,
            vec2_to_raylib(pos.pos),
            item.font_size,
            spacing,
            final_color,
        );
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
