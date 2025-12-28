//! Rendering system using Raylib.
//!
//! Draws sprites, optional debug overlays, and basic diagnostics each frame.
//! Renders to a fixed-resolution texture, then scales to fit the window with
//! letterboxing/pillarboxing to preserve aspect ratio.
//!
//! World-space rendering uses the shared [`Camera2DRes`] to transform between
//! world and screen coordinates.
use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::mapposition::MapPosition;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::debugmode::DebugMode;
use crate::resources::fontstore::FontStore;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::screensize::ScreenSize;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;

/// Bundled render resources to reduce system parameter count.
#[derive(SystemParam)]
pub struct RenderResources<'w> {
    pub camera: Res<'w, Camera2DRes>,
    pub screensize: Res<'w, ScreenSize>,
    pub window_size: Res<'w, WindowSize>,
    pub textures: Res<'w, TextureStore>,
    pub maybe_debug: Option<Res<'w, DebugMode>>,
}

/// Main render pass.
///
/// Contract
/// - Renders all game content to a fixed-resolution render target.
/// - Scales and blits the render target to the window with letterboxing.
/// - Uses `Camera2D` for world rendering, then overlays UI/debug in screen space.
/// - When `DebugMode` is present, draws additional information (entity counts,
///   camera parameters, and optional collider boxes/signals).
pub fn render_system(
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
    mut render_target: NonSendMut<RenderTarget>,
    res: RenderResources,
    query_map_sprites: Query<(
        &Sprite,
        &MapPosition,
        &ZIndex,
        Option<&Scale>,
        Option<&Rotation>,
    )>,
    query_colliders: Query<(&BoxCollider, &MapPosition)>,
    query_positions: Query<(&MapPosition, Option<&Signals>)>,
    query_map_dynamic_texts: Query<(&DynamicText, &MapPosition, &ZIndex)>,
    query_screen_dynamic_texts: Query<(&DynamicText, &ScreenPosition)>,
    query_screen_sprites: Query<(&Sprite, &ScreenPosition)>,
    fonts: NonSend<FontStore>,
    mut sprite_buffer: Local<Vec<(Sprite, MapPosition, ZIndex, Option<Scale>, Option<Rotation>)>>,
    mut text_buffer: Local<Vec<(DynamicText, MapPosition, ZIndex)>>,
) {
    // Unpack bundled resources for easier access
    let camera = &res.camera;
    let screensize = &res.screensize;
    let window_size = &res.window_size;
    let textures = &res.textures;
    let maybe_debug = &res.maybe_debug;

    // ========== PHASE 1: Render game content to the render target ==========
    {
        let mut d = rl.begin_texture_mode(&th, &mut render_target.texture);
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

            sprite_buffer.clear();
            sprite_buffer.extend(query_map_sprites.iter().filter_map(
                |(s, p, z, maybe_scale, maybe_rot)| {
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
                    overlap.then_some((s.clone(), *p, *z, maybe_scale.copied(), maybe_rot.copied()))
                },
            ));

            sprite_buffer.sort_unstable_by_key(|(_, _, z, _, _)| *z);
            for (sprite, pos, _z, maybe_scale, maybe_rot) in sprite_buffer.iter() {
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

                    let mut dest = Rectangle {
                        x: pos.pos.x,
                        y: pos.pos.y,
                        width: sprite.width,
                        height: sprite.height,
                    };

                    if let Some(scale) = maybe_scale {
                        dest.width *= scale.scale.x;
                        dest.height *= scale.scale.y;
                    }
                    let mut origin_scaled = Vector2 {
                        x: sprite.origin.x,
                        y: sprite.origin.y,
                    };
                    if let Some(scale) = maybe_scale {
                        origin_scaled.x *= scale.scale.x;
                        origin_scaled.y *= scale.scale.y;
                    }

                    let rotation = if let Some(rot) = maybe_rot {
                        rot.degrees
                    } else {
                        0.0
                    };

                    d2.draw_texture_pro(tex, src, dest, origin_scaled, rotation, Color::WHITE);
                }
            } // End sprite drawing in camera space

            text_buffer.clear();
            text_buffer.extend(query_map_dynamic_texts.iter().filter_map(|(t, p, z)| {
                let text_size = t.size();

                let min = Vector2 {
                    x: p.pos.x,
                    y: p.pos.y,
                };
                let max = Vector2 {
                    x: min.x + text_size.x,
                    y: min.y + text_size.y,
                };

                let overlap = !(max.x < view_min.x
                    || min.x > view_max.x
                    || max.y < view_min.y
                    || min.y > view_max.y);
                overlap.then_some((t.clone(), *p, *z))
            }));
            text_buffer.sort_unstable_by_key(|(_, _, z)| *z);
            for (text, pos, _z) in text_buffer.iter() {
                if let Some(font) = fonts.get(&text.font) {
                    d2.draw_text_ex(font, &text.text, pos.pos, text.font_size, 1.0, text.color);
                    if maybe_debug.is_some() {
                        d2.draw_rectangle_lines(
                            pos.pos.x as i32,
                            pos.pos.y as i32,
                            text.size().x as i32,
                            text.size().y as i32,
                            Color::ORANGE,
                        );
                    }
                }
            }

            if maybe_debug.is_some() {
                for (collider, position) in query_colliders.iter() {
                    let (x, y, w, h) = collider.get_aabb(position.pos);
                    d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
                }
                for (position, maybe_signals) in query_positions.iter() {
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

        // Draw in screen coordinates (UI layer) - still on the render target
        for (sprite, pos) in query_screen_sprites.iter() {
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

                d.draw_texture_pro(
                    tex,
                    src,
                    dest,
                    Vector2 {
                        x: sprite.origin.x,
                        y: sprite.origin.y,
                    },
                    0.0,
                    Color::WHITE,
                );
            }
            if maybe_debug.is_some() {
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

        for (text, pos) in query_screen_dynamic_texts.iter() {
            if let Some(font) = fonts.get(&text.font) {
                d.draw_text_ex(font, &text.text, pos.pos, text.font_size, 1.0, text.color);
                if maybe_debug.is_some() {
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

        if maybe_debug.is_some() {
            let debug_text = "DEBUG MODE (press F11 to toggle)";

            let fps = d.get_fps();
            let text = format!("{} | FPS: {}", debug_text, fps);
            d.draw_text(&text, 10, 10, 10, Color::GREENYELLOW);

            let entity_count = query_map_sprites.iter().count()
                + query_colliders.iter().count()
                + query_positions.iter().count();
            let text = format!("Map Sprites+colliders+positions: {}", entity_count);
            d.draw_text(&text, 10, 30, 10, Color::GREENYELLOW);

            let textures_count = textures.map.len();
            let text = format!("Loaded Textures: {}", textures_count);
            d.draw_text(&text, 10, 50, 10, Color::GREENYELLOW);

            let fonts_count = fonts.len();
            let text = format!("Loaded Fonts: {}", fonts_count);
            d.draw_text(&text, 10, 70, 10, Color::GREENYELLOW);

            let cam = &camera.0;
            let cam_text = format!(
                "Camera pos: ({:.1}, {:.1}) Zoom: {:.2}",
                cam.target.x, cam.target.y, cam.zoom
            );
            d.draw_text(
                &cam_text,
                10,
                (screensize.h - 30) as i32,
                10,
                Color::GREENYELLOW,
            );

            // Transform mouse from window space to game space for accurate display
            let window_mouse_pos = d.get_mouse_position();
            let game_mouse_pos = window_size.window_to_game_pos(
                window_mouse_pos,
                screensize.w as u32,
                screensize.h as u32,
            );
            let mouse_world = d.get_screen_to_world2D(game_mouse_pos, &camera.0);

            let mouse_text = format!(
                "Mouse game: ({:.1}, {:.1}) World: ({:.1}, {:.1})",
                game_mouse_pos.x, game_mouse_pos.y, mouse_world.x, mouse_world.y
            );

            d.draw_text(&mouse_text, 10, 90, 10, Color::GREENYELLOW);
        }
    } // End texture mode - render target is complete

    // ========== PHASE 2: Blit render target to window with letterboxing ==========
    {
        let mut d = rl.begin_drawing(&th);
        d.clear_background(Color::BLACK); // Black bars for letterboxing

        // Source rectangle (the entire render target, Y-flipped for OpenGL)
        let src = render_target.source_rect();

        // Destination rectangle (letterboxed to fit window)
        let dest = window_size.calculate_letterbox(
            render_target.game_width,
            render_target.game_height,
        );

        // Draw the render target scaled to fit the window
        d.draw_texture_pro(
            &render_target.texture,
            src,
            dest,
            Vector2 { x: 0.0, y: 0.0 },
            0.0,
            Color::WHITE,
        );
    }
}