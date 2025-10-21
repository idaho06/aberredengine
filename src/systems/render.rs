use bevy_ecs::prelude::*;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::mapposition::MapPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::debugmode::DebugMode;
use crate::resources::screensize::ScreenSize;
use crate::resources::texturestore::TextureStore;

pub fn render_system(
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    camera: Res<Camera2DRes>,
    querysprites: Query<(&Sprite, &MapPosition, &ZIndex)>,
    querycolliders: Query<(&BoxCollider, &MapPosition)>,
    querypositions: Query<(&MapPosition, Option<&Signals>)>,
    screensize: Res<ScreenSize>,
    textures: Res<TextureStore>,
    isdebug: Option<Res<DebugMode>>,
) {
    let mut d = rl.begin_drawing(&th);
    d.clear_background(Color::DARKGRAY);

    {
        // Draw in world coordinates using Camera2D.
        let mut d2 = d.begin_mode2D(camera.0);

        let tl = d2.get_screen_to_world2D(Vector2 { x: 0.0, y: 0.0 }, &camera.0);
        let br = d2.get_screen_to_world2D(
            Vector2 {
                x: screensize.w as f32,
                y: screensize.h as f32,
            },
            &camera.0,
        );
        let view_min = Vector2 {
            x: tl.x.min(br.x),
            y: tl.y.min(br.y),
        };
        let view_max = Vector2 {
            x: tl.x.max(br.x),
            y: tl.y.max(br.y),
        };

        let mut to_draw: Vec<(&Sprite, &MapPosition, &ZIndex)> = querysprites
            .iter()
            .filter_map(|(s, p, z)| {
                let min = Vector2 {
                    x: p.pos.x - s.origin.x,
                    y: p.pos.y - s.origin.y,
                };
                let max = Vector2 {
                    x: min.x + s.width,
                    y: min.y + s.height,
                };

                let overlap = !(max.x < view_min.x
                    || min.x > view_max.x
                    || max.y < view_min.y
                    || min.y > view_max.y);
                if overlap { Some((s, p, z)) } else { None }
            })
            .collect();
        to_draw.sort_by_key(|(_, _, z)| *z);
        for (sprite, pos, _z) in to_draw.iter() {
            if let Some(tex) = textures.get(sprite.tex_key.clone()) {
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

                d2.draw_texture_pro(tex, src, dest, sprite.origin, 0.0, Color::WHITE);
            }
        } // End sprite drawing
        if isdebug.is_some() {
            for (collider, position) in querycolliders.iter() {
                let (x, y, w, h) = collider.get_aabb(position.pos);

                d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
            }
            for (position, maybe_signals) in querypositions.iter() {
                d2.draw_line(
                    position.pos.x as i32 - 5,
                    position.pos.y as i32,
                    position.pos.x as i32 + 5,
                    position.pos.y as i32,
                    Color::GREEN,
                );
                d2.draw_line(
                    position.pos.x as i32,
                    position.pos.y as i32 - 5,
                    position.pos.x as i32,
                    position.pos.y as i32 + 5,
                    Color::GREEN,
                );
                if let Some(signals) = maybe_signals {
                    let mut y_offset = 10;
                    let font_size = 10;
                    let font_color = Color::YELLOW;
                    for flag in signals.get_flags() {
                        let text = format!("Flag: {}", flag);
                        d2.draw_text(
                            &text,
                            position.pos.x as i32 + 10,
                            position.pos.y as i32 + y_offset,
                            font_size,
                            font_color,
                        );
                        y_offset += 12;
                    }
                    for (key, value) in signals.get_scalars() {
                        let text = format!("Scalar: {} = {:.2}", key, value);
                        d2.draw_text(
                            &text,
                            position.pos.x as i32 + 10,
                            position.pos.y as i32 + y_offset,
                            font_size,
                            font_color,
                        );
                        y_offset += 12;
                    }
                    for (key, value) in signals.get_integers() {
                        let text = format!("Integer: {} = {}", key, value);
                        d2.draw_text(
                            &text,
                            position.pos.x as i32 + 10,
                            position.pos.y as i32 + y_offset,
                            font_size,
                            font_color,
                        );
                        y_offset += 12;
                    }
                }
            }
        } // End debug drawing
    } // End Camera2D mode
    if isdebug.is_some() {
        let debug_text = "DEBUG MODE (press F11 to toggle)";

        let fps = d.get_fps();
        let text = format!("{} | FPS: {}", debug_text, fps);
        d.draw_text(&text, 10, 10, 10, Color::BLACK);

        let entity_count = querysprites.iter().count()
            + querycolliders.iter().count()
            + querypositions.iter().count();
        let text = format!("Entities: {}", entity_count);
        d.draw_text(&text, 10, 30, 10, Color::BLACK);

        let cam = &camera.0;
        let cam_text = format!(
            "Camera pos: ({:.1}, {:.1}) Zoom: {:.2}",
            cam.target.x, cam.target.y, cam.zoom
        );
        d.draw_text(&cam_text, 10, (screensize.h - 30) as i32, 10, Color::BLACK);

        let mouse_pos = d.get_mouse_position();
        let mouse_world = d.get_screen_to_world2D(mouse_pos, &camera.0);

        let mouse_text = format!(
            "Mouse screen: ({:.1}, {:.1}) World: ({:.1}, {:.1})",
            mouse_pos.x, mouse_pos.y, mouse_world.x, mouse_world.y
        );

        d.draw_text(&mouse_text, 10, 70, 10, Color::BLACK);
    }
}
