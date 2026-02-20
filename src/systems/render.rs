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
use raylib::ffi;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::debugmode::DebugMode;
use crate::resources::gameconfig::GameConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::lua_runtime::UniformValue;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::worldtime::WorldTime;
use log::{warn, error};

type MapSpriteQueryData = (
    Entity,
    &'static Sprite,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static Scale>,
    Option<&'static Rotation>,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
);

type MapTextQueryData = (
    Entity,
    &'static DynamicText,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
);

type SpriteBufferItem = (
    Entity,
    Sprite,
    MapPosition,
    ZIndex,
    Option<Scale>,
    Option<Rotation>,
    Option<EntityShader>,
    Option<Tint>,
);

type TextBufferItem = (
    Entity,
    DynamicText,
    MapPosition,
    ZIndex,
    Option<EntityShader>,
    Option<Tint>,
);

/// Computed geometry for a sprite draw call via Raylib's `draw_texture_pro`.
///
/// In Raylib, `draw_texture_pro(tex, src, dest, origin, rotation, tint)` places
/// the texture so that local coordinate `(origin.x, origin.y)` maps to world
/// position `(dest.x, dest.y)`. Without rotation the visual top-left is at
/// `(dest.x - origin.x, dest.y - origin.y)`.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct SpriteRenderGeometry {
    pub dest: Rectangle,
    pub origin: Vector2,
    pub rotation: f32,
}

#[cfg(test)]
impl SpriteRenderGeometry {
    /// World-space position of the anchor/pivot (always `(dest.x, dest.y)`).
    pub fn anchor_world_pos(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x,
            y: self.dest.y,
        }
    }

    /// Visual top-left corner (ignoring rotation).
    pub fn visual_top_left(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x - self.origin.x,
            y: self.dest.y - self.origin.y,
        }
    }

    /// Visual bottom-right corner (ignoring rotation).
    pub fn visual_bottom_right(&self) -> Vector2 {
        Vector2 {
            x: self.dest.x - self.origin.x + self.dest.width,
            y: self.dest.y - self.origin.y + self.dest.height,
        }
    }
}

/// Pure geometry calculation for sprite rendering.
///
/// Computes the destination rectangle, scaled origin, and rotation that Raylib's
/// `draw_texture_pro` needs. Extracted from the render loop so it can be tested
/// without a GPU context.
pub(crate) fn compute_sprite_geometry(
    pos: &MapPosition,
    sprite: &Sprite,
    scale: Option<&Scale>,
    rot: Option<&Rotation>,
) -> SpriteRenderGeometry {
    let mut dest = Rectangle {
        x: pos.pos.x,
        y: pos.pos.y,
        width: sprite.width,
        height: sprite.height,
    };

    if let Some(scale) = scale {
        dest.width *= scale.scale.x;
        dest.height *= scale.scale.y;
    }

    let mut origin = Vector2 {
        x: sprite.origin.x,
        y: sprite.origin.y,
    };
    if let Some(scale) = scale {
        origin.x *= scale.scale.x;
        origin.y *= scale.scale.y;
    }

    let rotation = rot.map_or(0.0, |r| r.degrees);

    SpriteRenderGeometry {
        dest,
        origin,
        rotation,
    }
}

/// Compute the world-space AABB that fully contains the camera's visible area.
///
/// Converts all 4 screen corners to world space, then takes the min/max to form
/// a conservative bounding box. With a rotated camera, the 2-corner approach
/// (top-left + bottom-right) misses the other two corners which may extend
/// further, causing sprites near edges to be culled while still visible.
pub(crate) fn compute_view_bounds(
    screen_w: f32,
    screen_h: f32,
    camera: Camera2D,
    screen_to_world: impl Fn(Vector2, Camera2D) -> Vector2,
) -> (Vector2, Vector2) {
    let corners = [
        screen_to_world(Vector2 { x: 0.0, y: 0.0 }, camera),
        screen_to_world(Vector2 { x: screen_w, y: 0.0 }, camera),
        screen_to_world(Vector2 { x: 0.0, y: screen_h }, camera),
        screen_to_world(Vector2 { x: screen_w, y: screen_h }, camera),
    ];
    let view_min = Vector2 {
        x: corners[0].x.min(corners[1].x).min(corners[2].x).min(corners[3].x),
        y: corners[0].y.min(corners[1].y).min(corners[2].y).min(corners[3].y),
    };
    let view_max = Vector2 {
        x: corners[0].x.max(corners[1].x).max(corners[2].x).max(corners[3].x),
        y: corners[0].y.max(corners[1].y).max(corners[2].y).max(corners[3].y),
    };
    (view_min, view_max)
}

/// Compute the world-space AABB of a sprite for culling, accounting for scale and rotation.
///
/// For rotated sprites, uses a bounding circle (conservative but fast): the radius is the
/// distance from the anchor to the farthest corner of the scaled sprite, and the AABB is
/// expanded to contain that circle. For non-rotated sprites, returns the tight scaled AABB.
pub(crate) fn compute_sprite_cull_bounds(
    pos: &MapPosition,
    sprite: &Sprite,
    scale: Option<&Scale>,
    rot: Option<&Rotation>,
) -> (Vector2, Vector2) {
    let (sx, sy) = scale.map_or((1.0, 1.0), |s| (s.scale.x, s.scale.y));

    let scaled_w = sprite.width * sx;
    let scaled_h = sprite.height * sy;
    let scaled_ox = sprite.origin.x * sx;
    let scaled_oy = sprite.origin.y * sy;

    let is_rotated = rot.is_some_and(|r| r.degrees.abs() > f32::EPSILON);

    if is_rotated {
        // Bounding circle: radius = max distance from anchor to any corner of the scaled rect
        let corners = [
            (scaled_ox, scaled_oy),                         // top-left to anchor
            (scaled_w - scaled_ox, scaled_oy),              // top-right to anchor
            (scaled_ox, scaled_h - scaled_oy),              // bottom-left to anchor
            (scaled_w - scaled_ox, scaled_h - scaled_oy),   // bottom-right to anchor
        ];
        let radius = corners
            .iter()
            .map(|(dx, dy)| (dx * dx + dy * dy).sqrt())
            .fold(0.0_f32, f32::max);

        let min = Vector2 {
            x: pos.pos.x - radius,
            y: pos.pos.y - radius,
        };
        let max = Vector2 {
            x: pos.pos.x + radius,
            y: pos.pos.y + radius,
        };
        (min, max)
    } else {
        let min = Vector2 {
            x: pos.pos.x - scaled_ox,
            y: pos.pos.y - scaled_oy,
        };
        let max = Vector2 {
            x: min.x + scaled_w,
            y: min.y + scaled_h,
        };
        (min, max)
    }
}

/// Bundled render resources to reduce system parameter count.
#[derive(SystemParam)]
pub struct RenderResources<'w> {
    pub camera: Res<'w, Camera2DRes>,
    pub screensize: Res<'w, ScreenSize>,
    pub window_size: Res<'w, WindowSize>,
    pub textures: Res<'w, TextureStore>,
    pub world_time: Res<'w, WorldTime>,
    pub post_process: Res<'w, PostProcessShader>,
    pub config: Res<'w, GameConfig>,
    pub maybe_debug: Option<Res<'w, DebugMode>>,
    pub fonts: NonSend<'w, FontStore>,
}

/// Bundled queries for the render system.
#[derive(SystemParam)]
pub struct RenderQueries<'w, 's> {
    pub map_sprites: Query<'w, 's, MapSpriteQueryData>,
    pub colliders: Query<'w, 's, (&'static BoxCollider, &'static MapPosition)>,
    pub positions: Query<'w, 's, (&'static MapPosition, Option<&'static Signals>)>,
    pub map_texts: Query<'w, 's, MapTextQueryData>,
    pub rigidbodies: Query<'w, 's, &'static RigidBody>,
    pub screen_texts: Query<'w, 's, (&'static DynamicText, &'static ScreenPosition, Option<&'static Tint>)>,
    pub screen_sprites: Query<'w, 's, (&'static Sprite, &'static ScreenPosition, Option<&'static Tint>)>,
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
    mut raylib: crate::systems::RaylibAccess,
    mut render_target: NonSendMut<RenderTarget>,
    mut shader_store: NonSendMut<ShaderStore>,
    res: RenderResources,
    queries: RenderQueries,
    mut sprite_buffer: Local<Vec<SpriteBufferItem>>,
    mut text_buffer: Local<Vec<TextBufferItem>>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    let query_map_sprites = &queries.map_sprites;
    let query_colliders = &queries.colliders;
    let query_positions = &queries.positions;
    let query_map_dynamic_texts = &queries.map_texts;
    let query_rigidbodies = &queries.rigidbodies;
    let query_screen_dynamic_texts = &queries.screen_texts;
    let query_screen_sprites = &queries.screen_sprites;
    let fonts = &res.fonts;
    // Unpack bundled resources for easier access
    let camera = &res.camera;
    let screensize = &res.screensize;
    let window_size = &res.window_size;
    let textures = &res.textures;
    let maybe_debug = &res.maybe_debug;

    // ========== PHASE 1: Render game content to the render target ==========
    {
        let mut d = rl.begin_texture_mode(th, &mut render_target.texture);
        d.clear_background(res.config.background_color);

        {
            // Draw in world coordinates using Camera2D.
            let mut d2 = d.begin_mode2D(camera.0);

            let (view_min, view_max) = compute_view_bounds(
                screensize.w as f32,
                screensize.h as f32,
                camera.0,
                |pos, cam| d2.get_screen_to_world2D(pos, cam),
            );

            sprite_buffer.clear();
            sprite_buffer.extend(query_map_sprites.iter().filter_map(
                |(entity, s, p, z, maybe_scale, maybe_rot, maybe_shader, maybe_tint)| {
                    let (min, max) = compute_sprite_cull_bounds(
                        p,
                        s,
                        maybe_scale,
                        maybe_rot,
                    );

                    let overlap = !(max.x < view_min.x
                        || min.x > view_max.x
                        || max.y < view_min.y
                        || min.y > view_max.y);
                    overlap.then_some((
                        entity,
                        s.clone(),
                        *p,
                        *z,
                        maybe_scale.copied(),
                        maybe_rot.copied(),
                        maybe_shader.cloned(),
                        maybe_tint.copied(),
                    ))
                },
            ));

            // sprite_buffer.sort_unstable_by_key(|(_, _, _, z, _, _, _, _)| *z);
            sprite_buffer.sort_unstable_by(|a, b| {
                a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal)
            });
            for (entity, sprite, pos, _z, maybe_scale, maybe_rot, maybe_shader, maybe_tint) in
                sprite_buffer.iter()
            {
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

                    let geom = compute_sprite_geometry(
                        pos,
                        sprite,
                        maybe_scale.as_ref(),
                        maybe_rot.as_ref(),
                    );
                    let dest = geom.dest;
                    let origin_scaled = geom.origin;
                    let rotation = geom.rotation;

                    // Apply entity shader if present
                    if let Some(entity_shader) = maybe_shader {
                        if let Some(entry) = shader_store.get_mut(&entity_shader.shader_key) {
                            if entry.shader.is_shader_valid() {
                                // Set standard uniforms
                                set_standard_uniforms(
                                    &mut entry.shader,
                                    &mut entry.locations,
                                    &res.world_time,
                                    screensize,
                                    window_size,
                                    &dest,
                                );

                                // Set entity-specific uniforms
                                set_entity_uniforms(
                                    &mut entry.shader,
                                    &mut entry.locations,
                                    *entity,
                                    pos,
                                    maybe_rot.as_ref(),
                                    maybe_scale.as_ref(),
                                    sprite,
                                    query_rigidbodies,
                                );

                                // Set user-defined uniforms
                                for (name, value) in &entity_shader.uniforms {
                                    set_uniform_value(
                                        &mut entry.shader,
                                        &mut entry.locations,
                                        name,
                                        value,
                                    );
                                }

                                // Draw with shader
                                let tint_color =
                                    maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
                                let mut d_shader = d2.begin_shader_mode(&mut entry.shader);
                                d_shader.draw_texture_pro(
                                    tex,
                                    src,
                                    dest,
                                    origin_scaled,
                                    rotation,
                                    tint_color,
                                );
                            } else {
                                warn!("Entity shader '{}' is invalid, rendering without shader", entity_shader.shader_key);
                                let tint_color =
                                    maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
                                d2.draw_texture_pro(
                                    tex,
                                    src,
                                    dest,
                                    origin_scaled,
                                    rotation,
                                    tint_color,
                                );
                            }
                        } else {
                            warn!("Entity shader '{}' not found, rendering without shader", entity_shader.shader_key);
                            let tint_color = maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
                            d2.draw_texture_pro(
                                tex,
                                src,
                                dest,
                                origin_scaled,
                                rotation,
                                tint_color,
                            );
                        }
                    } else {
                        let tint_color = maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);
                        d2.draw_texture_pro(tex, src, dest, origin_scaled, rotation, tint_color);
                    }
                }
            } // End sprite drawing in camera space

            text_buffer.clear();
            text_buffer.extend(query_map_dynamic_texts.iter().filter_map(
                |(entity, t, p, z, maybe_shader, maybe_tint)| {
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
                    overlap.then_some((
                        entity,
                        t.clone(),
                        *p,
                        *z,
                        maybe_shader.cloned(),
                        maybe_tint.copied(),
                    ))
                },
            ));
            //text_buffer.sort_unstable_by_key(|(_, _, _, z, _, _)| *z);
            text_buffer.sort_unstable_by(|a, b| {
                a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal)
            });
            for (_entity, text, pos, _z, _maybe_shader, maybe_tint) in text_buffer.iter() {
                if let Some(font) = fonts.get(&text.font) {
                    let final_color = maybe_tint
                        .map(|t| t.multiply(text.color))
                        .unwrap_or(text.color);
                    d2.draw_text_ex(font, &text.text, pos.pos, text.font_size, 1.0, final_color);
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
        for (sprite, pos, maybe_tint) in query_screen_sprites.iter() {
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

        for (text, pos, maybe_tint) in query_screen_dynamic_texts.iter() {
            if let Some(font) = fonts.get(&text.font) {
                let final_color = maybe_tint
                    .map(|t| t.multiply(text.color))
                    .unwrap_or(text.color);
                d.draw_text_ex(font, &text.text, pos.pos, text.font_size, 1.0, final_color);
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
                screensize.h - 30,
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
            let mouse_world = d.get_screen_to_world2D(game_mouse_pos, camera.0);

            let mouse_text = format!(
                "Mouse game: ({:.1}, {:.1}) World: ({:.1}, {:.1})",
                game_mouse_pos.x, game_mouse_pos.y, mouse_world.x, mouse_world.y
            );

            d.draw_text(&mouse_text, 10, 90, 10, Color::GREENYELLOW);
        }
    } // End texture mode - render target is complete

    // ========== PHASE 2: Multi-pass post-processing and final blit ==========
    // Unpack additional resources
    let world_time = &res.world_time;
    let post_process = &res.post_process;

    // Source rectangle (the entire render target, Y-flipped for OpenGL)
    let src = render_target.source_rect();

    // Destination rectangle (letterboxed to fit window)
    let dest = window_size.calculate_letterbox(render_target.game_width, render_target.game_height);

    // Full-screen destination for intermediate passes (no letterboxing)
    let full_dest = Rectangle {
        x: 0.0,
        y: 0.0,
        width: render_target.game_width as f32,
        height: render_target.game_height as f32,
    };

    // Clone shader chain to avoid borrowing issues
    let shader_chain: Vec<_> = post_process.keys.to_vec();

    if shader_chain.is_empty() {
        // No post-processing - draw directly to window
        let mut d = rl.begin_drawing(th);
        d.clear_background(Color::BLACK);
        d.draw_texture_pro(
            &render_target.texture,
            src,
            dest,
            Vector2 { x: 0.0, y: 0.0 },
            0.0,
            Color::WHITE,
        );
    } else if shader_chain.len() == 1 {
        // Single shader - draw directly to window (existing behavior)
        let shader_key = &shader_chain[0];
        let mut use_shader = false;

        if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
            if entry.shader.is_shader_valid() {
                use_shader = true;

                // Set standard uniforms
                set_standard_uniforms(
                    &mut entry.shader,
                    &mut entry.locations,
                    world_time,
                    screensize,
                    window_size,
                    &dest,
                );

                // Set user uniforms
                for (name, value) in post_process.uniforms.iter() {
                    set_uniform_value(&mut entry.shader, &mut entry.locations, name, value);
                }
            } else {
                warn!("Post-process shader '{}' is invalid, rendering without shader", shader_key);
            }
        } else {
            warn!("Post-process shader '{}' not found, rendering without shader", shader_key);
        }

        let mut d = rl.begin_drawing(th);
        d.clear_background(Color::BLACK);

        if use_shader {
            if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                d_shader.draw_texture_pro(
                    &render_target.texture,
                    src,
                    dest,
                    Vector2 { x: 0.0, y: 0.0 },
                    0.0,
                    Color::WHITE,
                );
            }
        } else {
            d.draw_texture_pro(
                &render_target.texture,
                src,
                dest,
                Vector2 { x: 0.0, y: 0.0 },
                0.0,
                Color::WHITE,
            );
        }
    } else {
        // Multi-pass: ensure ping-pong buffers exist
        if let Err(e) = render_target.ensure_ping_pong_buffers(rl, th) {
            error!("Failed to create ping-pong buffers: {}", e);
            // Fallback: draw without shader
            let mut d = rl.begin_drawing(th);
            d.clear_background(Color::BLACK);
            d.draw_texture_pro(
                &render_target.texture,
                src,
                dest,
                Vector2 { x: 0.0, y: 0.0 },
                0.0,
                Color::WHITE,
            );
            return;
        }

        // Source buffer tracking: 0=main, 1=ping, 2=pong
        #[derive(Clone, Copy)]
        enum SourceBuffer {
            Main,
            Ping,
            Pong,
        }
        let mut source_buffer = SourceBuffer::Main;
        let mut valid_passes = 0;

        // Source rect with Y-flip for all textures
        let pass_src = Rectangle {
            x: 0.0,
            y: 0.0,
            width: render_target.game_width as f32,
            height: -(render_target.game_height as f32),
        };

        // Get raw pointers to independently borrow texture, ping, and pong
        // SAFETY: These fields are independent and don't alias
        let main_tex_ptr = &render_target.texture as *const RenderTexture2D;
        let ping_tex_ptr = render_target.ping.as_ref().unwrap() as *const RenderTexture2D;
        let pong_tex_ptr = render_target.pong.as_ref().unwrap() as *const RenderTexture2D;

        for (i, shader_key) in shader_chain.iter().enumerate() {
            let is_last_pass = i == shader_chain.len() - 1;

            // Validate shader exists and is valid
            let shader_valid = shader_store
                .get(shader_key.as_ref())
                .map(|e| e.shader.is_shader_valid())
                .unwrap_or(false);

            if !shader_valid {
                if shader_store.get(shader_key.as_ref()).is_none() {
                    warn!("Shader '{}' not found, skipping pass", shader_key);
                } else {
                    warn!("Shader '{}' invalid, skipping pass", shader_key);
                }
                continue;
            }

            // Set uniforms
            if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                set_standard_uniforms(
                    &mut entry.shader,
                    &mut entry.locations,
                    world_time,
                    screensize,
                    window_size,
                    &dest,
                );
                for (name, value) in post_process.uniforms.iter() {
                    set_uniform_value(&mut entry.shader, &mut entry.locations, name, value);
                }
            }

            // SAFETY: We're only reading from source_tex and writing to dest_tex,
            // and they never alias (main->ping, ping->pong, pong->ping, etc.)
            let source_tex: &RenderTexture2D = unsafe {
                match source_buffer {
                    SourceBuffer::Main => &*main_tex_ptr,
                    SourceBuffer::Ping => &*ping_tex_ptr,
                    SourceBuffer::Pong => &*pong_tex_ptr,
                }
            };

            if is_last_pass {
                // Draw to window
                let mut d = rl.begin_drawing(th);
                d.clear_background(Color::BLACK);

                if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                    let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                    d_shader.draw_texture_pro(
                        source_tex,
                        pass_src,
                        dest,
                        Vector2 { x: 0.0, y: 0.0 },
                        0.0,
                        Color::WHITE,
                    );
                }
            } else {
                // Draw to intermediate buffer
                // Choose destination buffer (opposite of source for ping-pong)
                let write_to_ping =
                    matches!(source_buffer, SourceBuffer::Main | SourceBuffer::Pong);

                if write_to_ping {
                    let dest_tex = render_target.ping.as_mut().unwrap();
                    let mut d = rl.begin_texture_mode(th, dest_tex);
                    d.clear_background(Color::BLACK);

                    if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                        let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                        d_shader.draw_texture_pro(
                            source_tex,
                            pass_src,
                            full_dest,
                            Vector2 { x: 0.0, y: 0.0 },
                            0.0,
                            Color::WHITE,
                        );
                    }
                    source_buffer = SourceBuffer::Ping;
                } else {
                    let dest_tex = render_target.pong.as_mut().unwrap();
                    let mut d = rl.begin_texture_mode(th, dest_tex);
                    d.clear_background(Color::BLACK);

                    if let Some(entry) = shader_store.get_mut(shader_key.as_ref()) {
                        let mut d_shader = d.begin_shader_mode(&mut entry.shader);
                        d_shader.draw_texture_pro(
                            source_tex,
                            pass_src,
                            full_dest,
                            Vector2 { x: 0.0, y: 0.0 },
                            0.0,
                            Color::WHITE,
                        );
                    }
                    source_buffer = SourceBuffer::Pong;
                }
            }

            valid_passes += 1;
        }

        // If no valid passes ran, draw without shader
        if valid_passes == 0 {
            let mut d = rl.begin_drawing(th);
            d.clear_background(Color::BLACK);
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
}

/// Set standard uniforms on a shader for post-processing.
///
/// Standard uniforms:
/// - uTime: elapsed time in seconds
/// - uDeltaTime: frame delta time in seconds
/// - uResolution: render target resolution (game resolution)
/// - uFrame: frame count
/// - uWindowResolution: window resolution
/// - uLetterbox: letterbox destination rectangle (x, y, w, h)
fn set_standard_uniforms(
    shader: &mut Shader,
    locations: &mut rustc_hash::FxHashMap<String, i32>,
    world_time: &WorldTime,
    screensize: &ScreenSize,
    window_size: &WindowSize,
    dest: &Rectangle,
) {
    // Helper to get or cache uniform location
    let get_loc =
        |shader: &Shader, locations: &mut rustc_hash::FxHashMap<String, i32>, name: &str| -> i32 {
            *locations
                .entry(name.to_string())
                .or_insert_with(|| shader.get_shader_location(name))
        };

    // uTime (float)
    let loc = get_loc(shader, locations, "uTime");
    if loc >= 0 {
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &world_time.elapsed as *const f32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
            );
        }
    }

    // uDeltaTime (float)
    let loc = get_loc(shader, locations, "uDeltaTime");
    if loc >= 0 {
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &world_time.delta as *const f32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
            );
        }
    }

    // uResolution (vec2) - game resolution
    let loc = get_loc(shader, locations, "uResolution");
    if loc >= 0 {
        let resolution = [screensize.w as f32, screensize.h as f32];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                resolution.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uFrame (int)
    let loc = get_loc(shader, locations, "uFrame");
    if loc >= 0 {
        let frame = world_time.frame_count as i32;
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &frame as *const i32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
            );
        }
    }

    // uWindowResolution (vec2)
    let loc = get_loc(shader, locations, "uWindowResolution");
    if loc >= 0 {
        let window_res = [window_size.w as f32, window_size.h as f32];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                window_res.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uLetterbox (vec4) - destination rectangle
    let loc = get_loc(shader, locations, "uLetterbox");
    if loc >= 0 {
        let letterbox = [dest.x, dest.y, dest.width, dest.height];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                letterbox.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4 as i32,
            );
        }
    }
}

/// Set a user-defined uniform value on a shader.
fn set_uniform_value(
    shader: &mut Shader,
    locations: &mut rustc_hash::FxHashMap<String, i32>,
    name: &str,
    value: &UniformValue,
) {
    let loc = *locations
        .entry(name.to_string())
        .or_insert_with(|| shader.get_shader_location(name));

    if loc < 0 {
        return; // Uniform not found in shader, silently skip
    }

    unsafe {
        match value {
            UniformValue::Float(v) => {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    v as *const f32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
                );
            }
            UniformValue::Int(v) => {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    v as *const i32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
                );
            }
            UniformValue::Vec2 { x, y } => {
                let vec = [*x, *y];
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
            UniformValue::Vec4 { x, y, z, w } => {
                let vec = [*x, *y, *z, *w];
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4 as i32,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Set entity-specific uniforms on a shader for per-entity rendering.
///
/// Entity-specific uniforms:
/// - uEntityId (int) - entity index
/// - uEntityPos (vec2) - world position
/// - uSpriteSize (vec2) - sprite dimensions
/// - uRotation (float) - rotation degrees (if present)
/// - uScale (vec2) - scale factor (if present)
/// - uVelocity (vec2) - velocity (if RigidBody present)
fn set_entity_uniforms(
    shader: &mut Shader,
    locations: &mut rustc_hash::FxHashMap<String, i32>,
    entity: Entity,
    pos: &MapPosition,
    rotation: Option<&Rotation>,
    scale: Option<&Scale>,
    sprite: &Sprite,
    rigidbody_query: &Query<&RigidBody>,
) {
    // Helper to get or cache uniform location
    let get_loc =
        |shader: &Shader, locations: &mut rustc_hash::FxHashMap<String, i32>, name: &str| -> i32 {
            *locations
                .entry(name.to_string())
                .or_insert_with(|| shader.get_shader_location(name))
        };

    // uEntityId (int) - use bits representation truncated to i32
    let loc = get_loc(shader, locations, "uEntityId");
    if loc >= 0 {
        let entity_id = (entity.to_bits() & 0xFFFFFFFF) as i32;
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                &entity_id as *const i32 as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
            );
        }
    }

    // uEntityPos (vec2)
    let loc = get_loc(shader, locations, "uEntityPos");
    if loc >= 0 {
        let entity_pos = [pos.pos.x, pos.pos.y];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                entity_pos.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uSpriteSize (vec2)
    let loc = get_loc(shader, locations, "uSpriteSize");
    if loc >= 0 {
        let sprite_size = [sprite.width, sprite.height];
        unsafe {
            ffi::SetShaderValue(
                **shader,
                loc,
                sprite_size.as_ptr() as *const std::ffi::c_void,
                ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }

    // uRotation (float) - only if Rotation component present
    if let Some(rot) = rotation {
        let loc = get_loc(shader, locations, "uRotation");
        if loc >= 0 {
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    &rot.degrees as *const f32 as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
                );
            }
        }
    }

    // uScale (vec2) - only if Scale component present
    if let Some(s) = scale {
        let loc = get_loc(shader, locations, "uScale");
        if loc >= 0 {
            let scale_vec = [s.scale.x, s.scale.y];
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    scale_vec.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
        }
    }

    // uVelocity (vec2) - only if RigidBody component present
    if let Ok(rb) = rigidbody_query.get(entity) {
        let loc = get_loc(shader, locations, "uVelocity");
        if loc >= 0 {
            let velocity = [rb.velocity.x, rb.velocity.y];
            unsafe {
                ffi::SetShaderValue(
                    **shader,
                    loc,
                    velocity.as_ptr() as *const std::ffi::c_void,
                    ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const EPSILON: f32 = 1e-6;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn make_sprite(w: f32, h: f32, origin_x: f32, origin_y: f32) -> Sprite {
        Sprite {
            tex_key: Arc::from("test"),
            width: w,
            height: h,
            offset: Vector2 { x: 0.0, y: 0.0 },
            origin: Vector2 {
                x: origin_x,
                y: origin_y,
            },
            flip_h: false,
            flip_v: false,
        }
    }

    // --- Anchor preservation tests ---

    #[test]
    fn anchor_preserved_with_center_origin() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);

        for scale_factor in [0.5_f32, 1.0, 2.0, 3.0, 10.0] {
            let scale = Scale::new(scale_factor, scale_factor);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 100.0) && approx_eq(anchor.y, 100.0),
                "Center origin: anchor drifted to ({}, {}) at scale {}",
                anchor.x,
                anchor.y,
                scale_factor
            );
        }
    }

    #[test]
    fn anchor_preserved_with_topleft_origin() {
        let pos = MapPosition::new(50.0, 75.0);
        let sprite = make_sprite(64.0, 48.0, 0.0, 0.0);

        for scale_factor in [0.25_f32, 1.0, 4.0] {
            let scale = Scale::new(scale_factor, scale_factor);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 50.0) && approx_eq(anchor.y, 75.0),
                "Top-left origin: anchor drifted to ({}, {}) at scale {}",
                anchor.x,
                anchor.y,
                scale_factor
            );
        }
    }

    #[test]
    fn anchor_preserved_with_arbitrary_origin() {
        let pos = MapPosition::new(200.0, 150.0);
        let sprite = make_sprite(32.0, 48.0, 10.0, 20.0);

        for (sx, sy) in [(1.0, 1.0), (2.0, 2.0), (0.5, 0.5), (3.0, 1.5)] {
            let scale = Scale::new(sx, sy);
            let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
            let anchor = geom.anchor_world_pos();
            assert!(
                approx_eq(anchor.x, 200.0) && approx_eq(anchor.y, 150.0),
                "Arbitrary origin: anchor drifted to ({}, {}) at scale ({}, {})",
                anchor.x,
                anchor.y,
                sx,
                sy
            );
        }
    }

    // --- Proportional scaling test ---

    #[test]
    fn visual_bounds_scale_proportionally() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 10.0, 10.0);

        let geom_1x = compute_sprite_geometry(&pos, &sprite, None, None);
        let tl_1x = geom_1x.visual_top_left();
        let br_1x = geom_1x.visual_bottom_right();

        // At 2x scale, distances from anchor to each edge should double
        let scale = Scale::new(2.0, 2.0);
        let geom_2x = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);
        let tl_2x = geom_2x.visual_top_left();
        let br_2x = geom_2x.visual_bottom_right();

        // Distance from anchor (100,100) to left edge
        let dist_left_1x = 100.0 - tl_1x.x; // 10
        let dist_left_2x = 100.0 - tl_2x.x; // should be 20
        assert!(
            approx_eq(dist_left_2x, dist_left_1x * 2.0),
            "Left edge distance: 1x={}, 2x={} (expected {})",
            dist_left_1x,
            dist_left_2x,
            dist_left_1x * 2.0
        );

        // Distance from anchor to right edge
        let dist_right_1x = br_1x.x - 100.0; // 22
        let dist_right_2x = br_2x.x - 100.0; // should be 44
        assert!(
            approx_eq(dist_right_2x, dist_right_1x * 2.0),
            "Right edge distance: 1x={}, 2x={} (expected {})",
            dist_right_1x,
            dist_right_2x,
            dist_right_1x * 2.0
        );

        // Distance from anchor to top edge
        let dist_top_1x = 100.0 - tl_1x.y;
        let dist_top_2x = 100.0 - tl_2x.y;
        assert!(
            approx_eq(dist_top_2x, dist_top_1x * 2.0),
            "Top edge distance: 1x={}, 2x={} (expected {})",
            dist_top_1x,
            dist_top_2x,
            dist_top_1x * 2.0
        );

        // Distance from anchor to bottom edge
        let dist_bottom_1x = br_1x.y - 100.0;
        let dist_bottom_2x = br_2x.y - 100.0;
        assert!(
            approx_eq(dist_bottom_2x, dist_bottom_1x * 2.0),
            "Bottom edge distance: 1x={}, 2x={} (expected {})",
            dist_bottom_1x,
            dist_bottom_2x,
            dist_bottom_1x * 2.0
        );
    }

    // --- Non-uniform scale ---

    #[test]
    fn non_uniform_scale_preserves_anchor() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let scale = Scale::new(2.0, 0.5);

        let geom = compute_sprite_geometry(&pos, &sprite, Some(&scale), None);

        // Anchor must stay at entity position
        let anchor = geom.anchor_world_pos();
        assert!(approx_eq(anchor.x, 100.0) && approx_eq(anchor.y, 100.0));

        // Width doubled, height halved
        assert!(approx_eq(geom.dest.width, 64.0));
        assert!(approx_eq(geom.dest.height, 16.0));

        // Origin scaled per-axis
        assert!(approx_eq(geom.origin.x, 32.0)); // 16 * 2
        assert!(approx_eq(geom.origin.y, 8.0)); // 16 * 0.5
    }

    // --- Identity / no-scale equivalence ---

    #[test]
    fn unit_scale_matches_no_scale() {
        let pos = MapPosition::new(42.0, 77.0);
        let sprite = make_sprite(24.0, 36.0, 8.0, 12.0);
        let unit = Scale::new(1.0, 1.0);

        let geom_none = compute_sprite_geometry(&pos, &sprite, None, None);
        let geom_unit = compute_sprite_geometry(&pos, &sprite, Some(&unit), None);

        assert!(approx_eq(geom_none.dest.x, geom_unit.dest.x));
        assert!(approx_eq(geom_none.dest.y, geom_unit.dest.y));
        assert!(approx_eq(geom_none.dest.width, geom_unit.dest.width));
        assert!(approx_eq(geom_none.dest.height, geom_unit.dest.height));
        assert!(approx_eq(geom_none.origin.x, geom_unit.origin.x));
        assert!(approx_eq(geom_none.origin.y, geom_unit.origin.y));
        assert!(approx_eq(geom_none.rotation, geom_unit.rotation));
    }

    // --- Rotation passthrough ---

    #[test]
    fn default_rotation_is_zero() {
        let pos = MapPosition::new(0.0, 0.0);
        let sprite = make_sprite(32.0, 32.0, 0.0, 0.0);
        let geom = compute_sprite_geometry(&pos, &sprite, None, None);
        assert!(approx_eq(geom.rotation, 0.0));
    }

    #[test]
    fn rotation_passes_through() {
        let pos = MapPosition::new(0.0, 0.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let rot = Rotation { degrees: 45.0 };
        let geom = compute_sprite_geometry(&pos, &sprite, None, Some(&rot));
        assert!(approx_eq(geom.rotation, 45.0));
    }

    // --- View bounds tests ---

    /// Mock screen_to_world: applies camera transform (translate + rotate + zoom) mathematically.
    fn mock_screen_to_world(screen_pos: Vector2, cam: Camera2D) -> Vector2 {
        // Reverse of Raylib's Camera2D: screen -> world
        // 1. Translate screen pos relative to camera offset
        let dx = screen_pos.x - cam.offset.x;
        let dy = screen_pos.y - cam.offset.y;
        // 2. Undo zoom
        let dx = dx / cam.zoom;
        let dy = dy / cam.zoom;
        // 3. Undo rotation (rotate by -rotation)
        let angle = -cam.rotation.to_radians();
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rx = dx * cos_a - dy * sin_a;
        let ry = dx * sin_a + dy * cos_a;
        // 4. Translate to world
        Vector2 {
            x: rx + cam.target.x,
            y: ry + cam.target.y,
        }
    }

    fn make_camera(target_x: f32, target_y: f32, offset_x: f32, offset_y: f32, rotation: f32, zoom: f32) -> Camera2D {
        Camera2D {
            target: Vector2 { x: target_x, y: target_y },
            offset: Vector2 { x: offset_x, y: offset_y },
            rotation,
            zoom,
        }
    }

    #[test]
    fn view_bounds_no_rotation() {
        // Camera centered at origin, offset at screen center, no rotation, zoom 1x
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // With no rotation, the 4-corner approach should match the 2-corner result exactly
        assert!(approx_eq(view_min.x, -400.0));
        assert!(approx_eq(view_min.y, -300.0));
        assert!(approx_eq(view_max.x, 400.0));
        assert!(approx_eq(view_max.y, 300.0));
    }

    #[test]
    fn view_bounds_45_degree_rotation() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 45.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // At 45°, the AABB should be larger than the unrotated screen rect
        let no_rot_cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (nr_min, nr_max) = compute_view_bounds(800.0, 600.0, no_rot_cam, mock_screen_to_world);

        let rotated_width = view_max.x - view_min.x;
        let unrotated_width = nr_max.x - nr_min.x;
        assert!(
            rotated_width > unrotated_width,
            "Rotated width {} should be larger than unrotated {}",
            rotated_width,
            unrotated_width,
        );

        let rotated_height = view_max.y - view_min.y;
        let unrotated_height = nr_max.y - nr_min.y;
        assert!(
            rotated_height > unrotated_height,
            "Rotated height {} should be larger than unrotated {}",
            rotated_height,
            unrotated_height,
        );
    }

    #[test]
    fn view_bounds_90_degree_rotation() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 90.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // At 90°, width and height effectively swap
        let rotated_width = view_max.x - view_min.x;
        let rotated_height = view_max.y - view_min.y;

        // Original screen: 800x600, so rotated AABB should be ~600 wide and ~800 tall
        // Use relaxed tolerance for trig floating point accumulation
        assert!(
            (rotated_width - 600.0).abs() < 0.001,
            "Rotated width {} should be ~600",
            rotated_width,
        );
        assert!(
            (rotated_height - 800.0).abs() < 0.001,
            "Rotated height {} should be ~800",
            rotated_height,
        );
    }

    #[test]
    fn view_bounds_with_zoom() {
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 2.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // Zoom 2x halves the world-space extents
        assert!(approx_eq(view_min.x, -200.0));
        assert!(approx_eq(view_min.y, -150.0));
        assert!(approx_eq(view_max.x, 200.0));
        assert!(approx_eq(view_max.y, 150.0));
    }

    // --- Sprite cull bounds tests ---

    #[test]
    fn sprite_cull_bounds_no_scale_no_rot() {
        let pos = MapPosition::new(100.0, 200.0);
        let sprite = make_sprite(32.0, 48.0, 16.0, 24.0);
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, None);

        // min = pos - origin, max = min + size
        assert!(approx_eq(min.x, 84.0));   // 100 - 16
        assert!(approx_eq(min.y, 176.0));  // 200 - 24
        assert!(approx_eq(max.x, 116.0));  // 84 + 32
        assert!(approx_eq(max.y, 224.0));  // 176 + 48
    }

    #[test]
    fn sprite_cull_bounds_with_scale() {
        let pos = MapPosition::new(100.0, 200.0);
        let sprite = make_sprite(32.0, 48.0, 16.0, 24.0);
        let scale = Scale::new(2.0, 2.0);
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, Some(&scale), None);

        // scaled: w=64, h=96, ox=32, oy=48
        assert!(approx_eq(min.x, 68.0));   // 100 - 32
        assert!(approx_eq(min.y, 152.0));  // 200 - 48
        assert!(approx_eq(max.x, 132.0));  // 68 + 64
        assert!(approx_eq(max.y, 248.0));  // 152 + 96
    }

    #[test]
    fn sprite_cull_bounds_with_rotation() {
        let pos = MapPosition::new(100.0, 100.0);
        let sprite = make_sprite(32.0, 32.0, 16.0, 16.0);
        let rot = Rotation { degrees: 45.0 };
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, Some(&rot));

        // Bounding circle radius = sqrt(16^2 + 16^2) = sqrt(512) ≈ 22.627
        let radius = (16.0_f32 * 16.0 + 16.0 * 16.0).sqrt();
        assert!(approx_eq(min.x, 100.0 - radius));
        assert!(approx_eq(min.y, 100.0 - radius));
        assert!(approx_eq(max.x, 100.0 + radius));
        assert!(approx_eq(max.y, 100.0 + radius));

        // The bounding circle AABB should be larger than the non-rotated AABB
        let (nr_min, nr_max) = compute_sprite_cull_bounds(&pos, &sprite, None, None);
        let rot_area = (max.x - min.x) * (max.y - min.y);
        let nr_area = (nr_max.x - nr_min.x) * (nr_max.y - nr_min.y);
        assert!(
            rot_area > nr_area,
            "Rotated bounds area {} should be larger than non-rotated {}",
            rot_area,
            nr_area,
        );
    }

    #[test]
    fn rotated_sprite_near_edge_not_culled() {
        // Regression test: a rotated sprite near the view edge should not be falsely culled.
        // Camera at origin, 800x600 screen, zoom 1x, no rotation.
        let cam = make_camera(0.0, 0.0, 400.0, 300.0, 0.0, 1.0);
        let (view_min, view_max) = compute_view_bounds(800.0, 600.0, cam, mock_screen_to_world);

        // Sprite at the right edge of view, rotated 45°. Its AABB center is just
        // outside the unscaled bounds but the bounding circle overlaps.
        let pos = MapPosition::new(410.0, 0.0);
        let sprite = make_sprite(64.0, 64.0, 32.0, 32.0);
        let rot = Rotation { degrees: 45.0 };
        let (min, max) = compute_sprite_cull_bounds(&pos, &sprite, None, Some(&rot));

        // The bounding circle radius = sqrt(32^2 + 32^2) ≈ 45.25
        // So min.x ≈ 410 - 45.25 = 364.75, which is < view_max.x = 400
        let overlap = !(max.x < view_min.x
            || min.x > view_max.x
            || max.y < view_min.y
            || min.y > view_max.y);
        assert!(
            overlap,
            "Rotated sprite near edge should not be culled. Sprite bounds: ({}, {}) - ({}, {}), View: ({}, {}) - ({}, {})",
            min.x, min.y, max.x, max.y, view_min.x, view_min.y, view_max.x, view_max.y,
        );
    }
}
