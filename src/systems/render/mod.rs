//! Rendering system using Raylib.
//!
//! Draws sprites, optional debug overlays, and basic diagnostics each frame.
//! Renders to a fixed-resolution texture, then scales to fit the window with
//! letterboxing/pillarboxing to preserve aspect ratio.
//!
//! World-space rendering uses the shared [`Camera2DRes`] to transform between
//! world and screen coordinates.
//!
//! When the active scene descriptor provides a [`GuiCallback`], an ImGui frame
//! is opened every render pass and the callback is invoked. This path is
//! independent of [`DebugMode`] and is intended for persistent game-developer UI
//! (HUDs, in-game editors, tool windows).

mod debug_overlay;
pub mod geometry;
mod postprocess;
mod sprite;
mod text;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
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
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugmode::DebugMode;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::input::InputState;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
use crate::resources::appstate::AppState;
use crate::resources::worldsignals::WorldSignals;
use crate::resources::worldtime::WorldTime;
use crate::systems::scene_dispatch::GuiCallback;
use log::warn;

use self::debug_overlay::draw_imgui_debug;
use self::geometry::{
    compute_sprite_cull_bounds, compute_sprite_geometry, compute_view_bounds,
    draw_rotated_rect_lines, resolve_world_transform,
};
use self::postprocess::{
    apply_postprocess_passes, set_entity_uniforms, set_standard_uniforms, set_uniform_value,
};
use self::sprite::draw_screen_sprites;
use self::text::draw_screen_texts;

type MapSpriteQueryData = (
    Entity,
    &'static Sprite,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static Scale>,
    Option<&'static Rotation>,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static GlobalTransform2D>,
);

type MapTextQueryData = (
    Entity,
    &'static DynamicText,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static GlobalTransform2D>,
);

pub(super) struct SpriteBufferItem {
    entity: Entity,
    sprite: Sprite,
    z_index: ZIndex,
    resolved_pos: MapPosition,
    resolved_scale: Option<Scale>,
    resolved_rot: Option<Rotation>,
    maybe_shader: Option<EntityShader>,
    maybe_tint: Option<Tint>,
}

pub(super) struct TextBufferItem {
    entity: Entity,
    text: DynamicText,
    z_index: ZIndex,
    resolved_pos: MapPosition,
    text_size: Vector2,
    maybe_shader: Option<EntityShader>,
    maybe_tint: Option<Tint>,
}

#[derive(Default)]
pub struct RenderLocals {
    sprite_buffer: Vec<SpriteBufferItem>,
    text_buffer: Vec<TextBufferItem>,
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
    pub colliders: Query<
        'w,
        's,
        (
            &'static BoxCollider,
            &'static MapPosition,
            Option<&'static GlobalTransform2D>,
        ),
    >,
    pub positions: Query<
        'w,
        's,
        (
            &'static MapPosition,
            Option<&'static Signals>,
            Option<&'static GlobalTransform2D>,
        ),
    >,
    pub map_texts: Query<'w, 's, MapTextQueryData>,
    pub rigidbodies: Query<'w, 's, &'static RigidBody>,
    pub screen_texts: Query<
        'w,
        's,
        (
            &'static DynamicText,
            &'static ScreenPosition,
            Option<&'static Tint>,
        ),
    >,
    pub screen_sprites: Query<
        'w,
        's,
        (
            &'static Sprite,
            &'static ScreenPosition,
            Option<&'static Tint>,
        ),
    >,
}

/// Extra resources needed for the imgui debug panels.
#[derive(SystemParam)]
pub(crate) struct DebugResources<'w> {
    pub world_signals: ResMut<'w, WorldSignals>,
    pub app_state: Res<'w, AppState>,
    pub input_state: Res<'w, InputState>,
    pub camera_follow: Res<'w, CameraFollowConfig>,
    pub scene_manager: Option<Res<'w, SceneManager>>,
    pub overlay_config: ResMut<'w, DebugOverlayConfig>,
}

/// Tracks which render buffer is the current source during multi-pass
/// post-processing (ping-pong pattern).
#[derive(Clone, Copy)]
pub(super) enum SourceBuffer {
    Main,
    Ping,
    Pong,
}

/// Main render pass.
///
/// Contract
/// - Renders all game content to a fixed-resolution render target.
/// - Scales and blits the render target to the window with letterboxing.
/// - Uses `Camera2D` for world rendering, then overlays UI/debug in screen space.
/// - When `DebugMode` is present, draws additional information (entity counts,
///   camera parameters, and optional collider boxes/signals).
/// - When the active scene's `gui_callback` is set, opens an ImGui frame and
///   calls it every frame, independent of debug mode.
#[allow(clippy::too_many_arguments, private_interfaces)]
pub fn render_system(
    mut raylib: crate::systems::RaylibAccess,
    mut render_target: NonSendMut<RenderTarget>,
    mut shader_store: NonSendMut<ShaderStore>,
    res: RenderResources,
    queries: RenderQueries,
    mut debug_res: DebugResources,
    mut locals: Local<RenderLocals>,
) {
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    let query_map_sprites = &queries.map_sprites;
    let query_colliders = &queries.colliders;
    let query_positions = &queries.positions;
    let query_map_dynamic_texts = &queries.map_texts;
    let query_rigidbodies = &queries.rigidbodies;
    let fonts = &res.fonts;
    let RenderLocals {
        sprite_buffer,
        text_buffer,
    } = &mut *locals;

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
                |(entity, s, p, z, maybe_scale, maybe_rot, maybe_shader, maybe_tint, maybe_gt)| {
                    let (resolved_pos, resolved_scale, resolved_rot) = resolve_world_transform(
                        *p,
                        maybe_scale.copied(),
                        maybe_rot.copied(),
                        maybe_gt.copied(),
                    );
                    let (min, max) = compute_sprite_cull_bounds(
                        &resolved_pos,
                        s,
                        resolved_scale.as_ref(),
                        resolved_rot.as_ref(),
                    );

                    let overlap = !(max.x < view_min.x
                        || min.x > view_max.x
                        || max.y < view_min.y
                        || min.y > view_max.y);
                    overlap.then_some(SpriteBufferItem {
                        entity,
                        sprite: s.clone(),
                        z_index: *z,
                        resolved_pos,
                        resolved_scale,
                        resolved_rot,
                        maybe_shader: maybe_shader.cloned(),
                        maybe_tint: maybe_tint.copied(),
                    })
                },
            ));

            // sprite_buffer.sort_unstable_by_key(|item| item.z_index);
            sprite_buffer.sort_unstable_by(|a, b| {
                a.z_index
                    .partial_cmp(&b.z_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for item in sprite_buffer.iter() {
                if let Some(tex) = textures.get(&item.sprite.tex_key) {
                    let mut src = Rectangle {
                        x: item.sprite.offset.x,
                        y: item.sprite.offset.y,
                        width: item.sprite.width,
                        height: item.sprite.height,
                    };
                    if item.sprite.flip_h {
                        src.width = -src.width;
                    }
                    if item.sprite.flip_v {
                        src.height = -src.height;
                    }

                    let geom = compute_sprite_geometry(
                        &item.resolved_pos,
                        &item.sprite,
                        item.resolved_scale.as_ref(),
                        item.resolved_rot.as_ref(),
                    );
                    let dest = geom.dest;
                    let origin_scaled = geom.origin;
                    let rotation = geom.rotation;

                    let tint_color = item.maybe_tint.map(|t| t.color).unwrap_or(Color::WHITE);

                    // Apply entity shader if present
                    if let Some(entity_shader) = &item.maybe_shader {
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
                                    item.entity,
                                    &item.resolved_pos,
                                    item.resolved_rot.as_ref(),
                                    item.resolved_scale.as_ref(),
                                    Vector2 {
                                        x: item.sprite.width,
                                        y: item.sprite.height,
                                    },
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
                                warn!(
                                    "Entity shader '{}' is invalid, rendering without shader",
                                    entity_shader.shader_key
                                );
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
                            warn!(
                                "Entity shader '{}' not found, rendering without shader",
                                entity_shader.shader_key
                            );
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
                        d2.draw_texture_pro(tex, src, dest, origin_scaled, rotation, tint_color);
                    }

                    if maybe_debug.is_some() && debug_res.overlay_config.show_sprite_bounds {
                        draw_rotated_rect_lines(
                            &mut d2,
                            dest,
                            origin_scaled,
                            rotation,
                            Color::BLUE,
                        );
                    }
                }
            }

            text_buffer.clear();
            text_buffer.extend(query_map_dynamic_texts.iter().filter_map(
                |(entity, t, p, z, maybe_shader, maybe_tint, maybe_gt)| {
                    let resolved_pos = MapPosition {
                        pos: maybe_gt.map_or(p.pos, |gt| gt.position),
                    };
                    let text_size = t.size();
                    let min = resolved_pos.pos;
                    let max = Vector2 {
                        x: min.x + text_size.x,
                        y: min.y + text_size.y,
                    };

                    let overlap = !(max.x < view_min.x
                        || min.x > view_max.x
                        || max.y < view_min.y
                        || min.y > view_max.y);
                    overlap.then_some(TextBufferItem {
                        entity,
                        text: t.clone(),
                        z_index: *z,
                        resolved_pos,
                        text_size,
                        maybe_shader: maybe_shader.cloned(),
                        maybe_tint: maybe_tint.copied(),
                    })
                },
            ));
            text_buffer.sort_unstable_by(|a, b| {
                a.z_index
                    .partial_cmp(&b.z_index)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for item in text_buffer.iter() {
                if let Some(font) = fonts.get(&item.text.font) {
                    let final_color = item
                        .maybe_tint
                        .map(|t| t.multiply(item.text.color))
                        .unwrap_or(item.text.color);

                    if let Some(entity_shader) = &item.maybe_shader {
                        if let Some(entry) = shader_store.get_mut(&entity_shader.shader_key) {
                            if entry.shader.is_shader_valid() {
                                let dest = Rectangle {
                                    x: item.resolved_pos.pos.x,
                                    y: item.resolved_pos.pos.y,
                                    width: item.text_size.x,
                                    height: item.text_size.y,
                                };
                                set_standard_uniforms(
                                    &mut entry.shader,
                                    &mut entry.locations,
                                    &res.world_time,
                                    screensize,
                                    window_size,
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
                                    query_rigidbodies,
                                );
                                for (name, value) in &entity_shader.uniforms {
                                    set_uniform_value(
                                        &mut entry.shader,
                                        &mut entry.locations,
                                        name,
                                        value,
                                    );
                                }
                                let mut d_shader = d2.begin_shader_mode(&mut entry.shader);
                                d_shader.draw_text_ex(
                                    font,
                                    &item.text.text,
                                    item.resolved_pos.pos,
                                    item.text.font_size,
                                    1.0,
                                    final_color,
                                );
                            } else {
                                warn!(
                                    "Entity shader '{}' is invalid, rendering without shader",
                                    entity_shader.shader_key
                                );
                                d2.draw_text_ex(
                                    font,
                                    &item.text.text,
                                    item.resolved_pos.pos,
                                    item.text.font_size,
                                    1.0,
                                    final_color,
                                );
                            }
                        } else {
                            warn!(
                                "Entity shader '{}' not found, rendering without shader",
                                entity_shader.shader_key
                            );
                            d2.draw_text_ex(
                                font,
                                &item.text.text,
                                item.resolved_pos.pos,
                                item.text.font_size,
                                1.0,
                                final_color,
                            );
                        }
                    } else {
                        d2.draw_text_ex(
                            font,
                            &item.text.text,
                            item.resolved_pos.pos,
                            item.text.font_size,
                            1.0,
                            final_color,
                        );
                    }

                    if maybe_debug.is_some() && debug_res.overlay_config.show_text_bounds {
                        d2.draw_rectangle_lines(
                            item.resolved_pos.pos.x as i32,
                            item.resolved_pos.pos.y as i32,
                            item.text_size.x as i32,
                            item.text_size.y as i32,
                            Color::ORANGE,
                        );
                    }
                }
            }

            if maybe_debug.is_some() {
                if debug_res.overlay_config.show_collider_boxes {
                    for (collider, position, maybe_gt) in query_colliders.iter() {
                        let world_pos = maybe_gt.map_or(position.pos, |gt| gt.position);
                        let (x, y, w, h) = collider.get_aabb(world_pos);
                        d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
                    }
                }
                if debug_res.overlay_config.show_position_crosshairs
                    || debug_res.overlay_config.show_entity_signals
                {
                    for (position, maybe_signals, maybe_gt) in query_positions.iter() {
                        let world_pos = maybe_gt.map_or(position.pos, |gt| gt.position);
                        if debug_res.overlay_config.show_position_crosshairs {
                            d2.draw_line(
                                world_pos.x as i32 - 5,
                                world_pos.y as i32,
                                world_pos.x as i32 + 5,
                                world_pos.y as i32,
                                Color::GREEN,
                            );
                            d2.draw_line(
                                world_pos.x as i32,
                                world_pos.y as i32 - 5,
                                world_pos.x as i32,
                                world_pos.y as i32 + 5,
                                Color::GREEN,
                            );
                        }
                        if debug_res.overlay_config.show_entity_signals
                            && let Some(signals) = maybe_signals
                        {
                            let mut y_offset = 10;
                            let font_size = 10;
                            let font_color = Color::YELLOW;
                            for flag in signals.get_flags() {
                                let text = format!("Flag: {}", flag);
                                d2.draw_text(
                                    &text,
                                    world_pos.x as i32 + 10,
                                    world_pos.y as i32 + y_offset,
                                    font_size,
                                    font_color,
                                );
                                y_offset += 12;
                            }
                            for (key, value) in signals.get_scalars() {
                                let text = format!("Scalar: {} = {:.2}", key, value);
                                d2.draw_text(
                                    &text,
                                    world_pos.x as i32 + 10,
                                    world_pos.y as i32 + y_offset,
                                    font_size,
                                    font_color,
                                );
                                y_offset += 12;
                            }
                            for (key, value) in signals.get_integers() {
                                let text = format!("Integer: {} = {}", key, value);
                                d2.draw_text(
                                    &text,
                                    world_pos.x as i32 + 10,
                                    world_pos.y as i32 + y_offset,
                                    font_size,
                                    font_color,
                                );
                                y_offset += 12;
                            }
                        }
                    }
                }
            }
        }

        // Draw in screen coordinates (UI layer) - still on the render target
        let debug = maybe_debug.is_some();
        let debug_sprites = debug && debug_res.overlay_config.show_sprite_bounds;
        let debug_texts = debug && debug_res.overlay_config.show_text_bounds;
        draw_screen_sprites(&mut d, &queries.screen_sprites, textures, debug_sprites);
        draw_screen_texts(&mut d, &queries.screen_texts, fonts, debug_texts);
    }

    // ========== PHASE 2: Multi-pass post-processing and final blit ==========
    let debug_active = maybe_debug.is_some();

    // Extract gui_callback from the active scene (fn pointer is Copy — no borrow held).
    // Must be done before taking mutable borrows of other debug_res fields below.
    let gui_callback: Option<GuiCallback> = debug_res
        .scene_manager
        .as_deref()
        .and_then(|sm| sm.active_scene.as_deref().and_then(|name| sm.get(name)))
        .and_then(|desc| desc.gui_callback);

    let needs_imgui = debug_active || gui_callback.is_some();

    if needs_imgui {
        // Debug-only values — computed only when needed
        let (
            fps,
            game_mouse_pos,
            mouse_world,
            sprite_count,
            collider_count,
            position_count,
            rigidbody_count,
            screen_sprite_count,
            screen_text_count,
            shader_count,
        ) = if debug_active {
            let fps = rl.get_fps();
            let window_mouse_pos = rl.get_mouse_position();
            let game_mouse_pos = window_size.window_to_game_pos(
                window_mouse_pos,
                screensize.w as u32,
                screensize.h as u32,
            );
            let mouse_world = rl.get_screen_to_world2D(game_mouse_pos, camera.0);
            let sprite_count = queries.map_sprites.iter().count();
            let collider_count = queries.colliders.iter().count();
            let position_count = queries.positions.iter().count();
            let rigidbody_count = queries.rigidbodies.iter().count();
            let screen_sprite_count = queries.screen_sprites.iter().count();
            let screen_text_count = queries.screen_texts.iter().count();
            let shader_count = shader_store.len();
            (
                fps,
                game_mouse_pos,
                mouse_world,
                sprite_count,
                collider_count,
                position_count,
                rigidbody_count,
                screen_sprite_count,
                screen_text_count,
                shader_count,
            )
        } else {
            // Dummy values — only reached when gui_callback is Some; debug_active is false
            // so the debug branch inside the closure will not execute them.
            (0, Vector2::zero(), Vector2::zero(), 0, 0, 0, 0, 0, 0, 0)
        };

        // Extract refs before closure (avoids borrow conflict with apply_postprocess_passes)
        let overlay_config = &mut *debug_res.overlay_config;
        let world_signals = &mut *debug_res.world_signals;
        let app_state = &*debug_res.app_state;
        let input_state = &*debug_res.input_state;
        let camera_follow = &*debug_res.camera_follow;
        let scene_manager = debug_res.scene_manager.as_deref();
        let world_time = &*res.world_time;
        let config = &*res.config;

        let closure = move |d: &RaylibDrawHandle<'_>| {
            use raylib::imgui::RayImGUITrait;
            let Some(ui) = d.begin_imgui() else { return };

            if debug_active {
                draw_imgui_debug(
                    &ui,
                    overlay_config,
                    world_signals,
                    input_state,
                    camera,
                    camera_follow,
                    scene_manager,
                    textures,
                    fonts,
                    shader_count,
                    screensize,
                    window_size,
                    world_time,
                    config,
                    fps,
                    sprite_count,
                    collider_count,
                    position_count,
                    rigidbody_count,
                    screen_sprite_count,
                    screen_text_count,
                    game_mouse_pos,
                    mouse_world,
                );
            }

            if let Some(cb) = gui_callback {
                cb(&ui, world_signals, textures, fonts, app_state);
            }
        };
        apply_postprocess_passes(
            rl,
            th,
            &mut render_target,
            &mut shader_store,
            &res.post_process,
            world_time,
            &res.screensize,
            &res.window_size,
            Some(closure),
        );
    } else {
        apply_postprocess_passes(
            rl,
            th,
            &mut render_target,
            &mut shader_store,
            &res.post_process,
            &res.world_time,
            &res.screensize,
            &res.window_size,
            None::<fn(&RaylibDrawHandle<'_>)>,
        );
    }
}
