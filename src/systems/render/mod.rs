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
mod gui_panel;
mod postprocess;
mod sprite;
mod text;

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::*;

use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::guibutton::{GuiButton, GuiWidgetState};
use crate::components::guiwindow::GuiWindow;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::resources::appstate::AppState;
use crate::resources::camera2d::Camera2DRes;
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugmode::DebugMode;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::fontstore::FontStore;
use crate::resources::gameconfig::GameConfig;
use crate::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiTheme};
use crate::resources::imgui_bridge::ImguiBridge;
use crate::resources::input::InputState;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::rendertarget::RenderTarget;
use crate::resources::scenemanager::SceneManager;
use crate::resources::screensize::ScreenSize;
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::windowsize::WindowSize;
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
use self::gui_panel::draw_screen_panel_item;
use self::sprite::draw_screen_sprite_item;
use self::text::draw_screen_text_item;

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

/// Screen-space sprite draw item. Simpler than [`SpriteBufferItem`]: screen-space
/// has no Scale/Rotation/GlobalTransform2D resolution, no `EntityShader` support
/// (screen-space shaders are out of scope), and no view-bounds culling.
pub(super) struct ScreenSpriteBufferItem {
    sprite: Sprite,
    z_index: ZIndex,
    pos: ScreenPosition,
    maybe_tint: Option<Tint>,
}

/// Screen-space text draw item. Mirrors [`ScreenSpriteBufferItem`]'s simplicity.
///
/// Stores only the fields [`draw_screen_text_item`](text::draw_screen_text_item)
/// actually reads, rather than a full [`DynamicText`] clone — `DynamicText` also
/// carries `initial_text`/`initial_color`, which exist for editor round-tripping
/// and are never read at draw time. Avoiding them keeps this struct (and thus
/// every element of the [`ScreenDrawItem`] enum it's wrapped in, sprites
/// included) smaller, which matters for cache density when sorting/iterating
/// tens of thousands of items per frame.
pub(super) struct ScreenTextBufferItem {
    text: Arc<str>,
    font: Arc<str>,
    font_size: f32,
    color: Color,
    size: Vector2,
    z_index: ZIndex,
    pos: ScreenPosition,
    maybe_tint: Option<Tint>,
}

/// Screen-space GUI window panel draw item. Window backgrounds sit below
/// sprites/text drawn on top of them (see [`ScreenDrawItem::variant_rank`]).
pub(super) struct ScreenPanelBufferItem {
    panel: GuiNinePatch,
    size: Vector2,
    z_index: ZIndex,
    pos: ScreenPosition,
}

/// Tagged union of screen-space draw items, sorted together by [`ZIndex`] into
/// one dispatch order. A future GUI refactor can add variants here (e.g.
/// NPatch panel/button) — doing so touches this enum plus one match arm each
/// in [`ScreenDrawItem::z_index`], [`ScreenDrawItem::variant_rank`], the
/// collect step, and the dispatch loop in [`draw_screen_space`]; it does not
/// require restructuring the sort/dispatch skeleton itself.
pub(super) enum ScreenDrawItem {
    Panel(ScreenPanelBufferItem),
    Sprite(ScreenSpriteBufferItem),
    Text(ScreenTextBufferItem),
}

impl ScreenDrawItem {
    fn z_index(&self) -> ZIndex {
        match self {
            ScreenDrawItem::Panel(p) => p.z_index,
            ScreenDrawItem::Sprite(s) => s.z_index,
            ScreenDrawItem::Text(t) => t.z_index,
        }
    }

    /// Secondary sort key, used only to break ties at equal `z_index`: panel
    /// backgrounds (0) sort below sprites (1), which sort below text (2), so
    /// a caption draws on top of its own widget's background. Encoding the
    /// tie-break here (rather than relying on `sort_by`'s stability +
    /// insertion order) lets the buffer use the faster in-place
    /// `sort_unstable_by` instead of an allocating stable sort.
    fn variant_rank(&self) -> u8 {
        match self {
            ScreenDrawItem::Panel(_) => 0,
            ScreenDrawItem::Sprite(_) => 1,
            ScreenDrawItem::Text(_) => 2,
        }
    }

    /// Draw-order comparator: ascending `z_index`, then `variant_rank` as the
    /// tie-break. Shared by `draw_screen_space` and its tests so the two
    /// can't drift apart.
    fn cmp_draw_order(a: &Self, b: &Self) -> std::cmp::Ordering {
        a.z_index()
            .partial_cmp(&b.z_index())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.variant_rank().cmp(&b.variant_rank()))
    }
}

#[derive(Default)]
pub struct RenderLocals {
    sprite_buffer: Vec<SpriteBufferItem>,
    text_buffer: Vec<TextBufferItem>,
    screen_draw_buffer: Vec<ScreenDrawItem>,
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
    pub gui_theme: Option<Res<'w, GuiTheme>>,
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
            &'static ZIndex,
            Option<&'static Tint>,
        ),
    >,
    pub screen_sprites: Query<
        'w,
        's,
        (
            &'static Sprite,
            &'static ScreenPosition,
            &'static ZIndex,
            Option<&'static Tint>,
        ),
    >,
    pub gui_windows: Query<'w, 's, (&'static GuiWindow, &'static ScreenPosition, &'static ZIndex)>,
    pub gui_buttons: Query<'w, 's, (&'static GuiButton, &'static ScreenPosition, &'static ZIndex)>,
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
    mut imgui_bridge: NonSendMut<ImguiBridge>,
    mut shader_store: NonSendMut<ShaderStore>,
    res: RenderResources,
    queries: RenderQueries,
    mut debug_res: DebugResources,
    mut locals: Local<RenderLocals>,
) {
    crate::tracy::tracy_span!("render_system");
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
        screen_draw_buffer,
    } = &mut *locals;

    // Unpack bundled resources for easier access
    let camera = &res.camera;
    let screensize = &res.screensize;
    let window_size = &res.window_size;
    let textures = &res.textures;
    let maybe_debug = &res.maybe_debug;

    // ========== PHASE 1: Render game content to the render target ==========
    {
        crate::tracy::tracy_span!("render/to_texture");
        let mut d = rl.begin_texture_mode(th, &mut render_target.texture);
        d.clear_background(res.config.background_color);

        {
            // Draw in world coordinates using Camera2D.
            crate::tracy::tracy_span!("render/world_space");
            let render_cam = if res.config.pixel_snap_camera {
                camera.pixel_snapped()
            } else {
                camera.0
            };
            let mut d2 = d.begin_mode2D(render_cam);

            let (view_min, view_max) = compute_view_bounds(
                screensize.w as f32,
                screensize.h as f32,
                render_cam,
                |pos, cam| d2.get_screen_to_world2D(pos, cam),
            );

            {
                crate::tracy::tracy_span!("render/build_sprite_buffer");
                sprite_buffer.clear();
                sprite_buffer.extend(query_map_sprites.iter().filter_map(
                    |(
                        entity,
                        s,
                        p,
                        z,
                        maybe_scale,
                        maybe_rot,
                        maybe_shader,
                        maybe_tint,
                        maybe_gt,
                    )| {
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
            } // build_sprite_buffer
            {
                crate::tracy::tracy_span!("render/draw_world_sprites");
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
                                    for (name, value) in entity_shader.uniforms.iter() {
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
                            d2.draw_texture_pro(
                                tex,
                                src,
                                dest,
                                origin_scaled,
                                rotation,
                                tint_color,
                            );
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
            } // draw_world_sprites

            {
                crate::tracy::tracy_span!("render/build_text_buffer");
                text_buffer.clear();
                text_buffer.extend(query_map_dynamic_texts.iter().filter_map(
                    |(entity, t, p, z, maybe_shader, maybe_tint, maybe_gt)| {
                        let resolved_pos =
                            MapPosition::from_vec(maybe_gt.map_or(p.pos, |gt| gt.position));
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
            } // build_text_buffer
            {
                crate::tracy::tracy_span!("render/draw_world_texts");
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
                                    for (name, value) in entity_shader.uniforms.iter() {
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
            } // draw_world_texts

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

            if let Some(cb) = debug_res
                .scene_manager
                .as_deref()
                .and_then(|sm| sm.active_scene.as_deref().and_then(|name| sm.get(name)))
                .and_then(|desc| desc.world_draw_callback)
            {
                let app_state = &*debug_res.app_state;
                let world_signals = &*debug_res.world_signals;
                cb(
                    &mut d2,
                    &camera.0,
                    &res.screensize,
                    app_state,
                    world_signals,
                );
            }
        }

        // Draw in screen coordinates (UI layer) - still on the render target
        let debug = maybe_debug.is_some();
        let debug_sprites = debug && debug_res.overlay_config.show_sprite_bounds;
        let debug_texts = debug && debug_res.overlay_config.show_text_bounds;
        {
            crate::tracy::tracy_span!("render/screen_space");
            draw_screen_space(
                &mut d,
                &queries.screen_sprites,
                &queries.screen_texts,
                &queries.gui_windows,
                &queries.gui_buttons,
                res.gui_theme.as_deref(),
                textures,
                fonts,
                screen_draw_buffer,
                debug_sprites,
                debug_texts,
            );
        }
    }

    // ========== PHASE 2: Multi-pass post-processing and final blit ==========
    crate::tracy::tracy_span!("render/postprocess");
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

        let closure = move |_d: &RaylibDrawHandle<'_>| {
            imgui_bridge.render(|ui| {
                if debug_active {
                    draw_imgui_debug(
                        ui,
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
                    cb(ui, world_signals, textures, fonts, app_state);
                }
            });
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

/// Collects screen-space sprites and texts into one merged buffer, sorts by
/// [`ZIndex`], and dispatches draw calls in that order.
///
/// Uses the same in-place `sort_unstable_by` as the world-space buffers — the
/// equal-z tie-break (text drawn on top of a same-z sprite, the sane default
/// for UI captions over panel backgrounds) is encoded directly in the
/// comparator via [`ScreenDrawItem::variant_rank`] instead of relying on
/// `sort_by`'s stability and a fixed collection order. This keeps the merged,
/// heterogeneous buffer on the cheaper allocation-free sort even though it
/// holds two item types, which matters once this buffer holds tens of
/// thousands of items (e.g. a screen-space bunnymark-style stress scene).
/// Selects the nine-patch for a `GuiButton`'s current state from its skin.
fn resolve_button_patch(skin: &GuiButtonSkin, state: GuiWidgetState) -> &GuiNinePatch {
    match state {
        GuiWidgetState::Normal => &skin.normal,
        GuiWidgetState::Hovered => &skin.hover,
        GuiWidgetState::Pressed => &skin.pressed,
        GuiWidgetState::Disabled => &skin.disabled,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_screen_space(
    d: &mut impl RaylibDraw,
    screen_sprites: &Query<(&Sprite, &ScreenPosition, &ZIndex, Option<&Tint>)>,
    screen_texts: &Query<(&DynamicText, &ScreenPosition, &ZIndex, Option<&Tint>)>,
    gui_windows: &Query<(&GuiWindow, &ScreenPosition, &ZIndex)>,
    gui_buttons: &Query<(&GuiButton, &ScreenPosition, &ZIndex)>,
    gui_theme: Option<&GuiTheme>,
    textures: &TextureStore,
    fonts: &FontStore,
    buffer: &mut Vec<ScreenDrawItem>,
    debug_sprites: bool,
    debug_texts: bool,
) {
    buffer.clear();
    if let Some(theme) = gui_theme {
        let panel = &theme.panel;
        buffer.extend(gui_windows.iter().map(|(w, p, z)| {
            ScreenDrawItem::Panel(ScreenPanelBufferItem {
                panel: panel.clone(),
                size: w.size,
                z_index: *z,
                pos: *p,
            })
        }));
        if let Some(skin) = &theme.button {
            buffer.extend(gui_buttons.iter().map(|(b, p, z)| {
                ScreenDrawItem::Panel(ScreenPanelBufferItem {
                    panel: resolve_button_patch(skin, b.state).clone(),
                    size: b.size,
                    z_index: *z,
                    pos: *p,
                })
            }));
        }
    }
    buffer.extend(screen_sprites.iter().map(|(s, p, z, maybe_tint)| {
        ScreenDrawItem::Sprite(ScreenSpriteBufferItem {
            sprite: s.clone(),
            z_index: *z,
            pos: *p,
            maybe_tint: maybe_tint.copied(),
        })
    }));
    buffer.extend(screen_texts.iter().map(|(t, p, z, maybe_tint)| {
        ScreenDrawItem::Text(ScreenTextBufferItem {
            text: Arc::clone(&t.text),
            font: Arc::clone(&t.font),
            font_size: t.font_size,
            color: t.color,
            size: t.size(),
            z_index: *z,
            pos: *p,
            maybe_tint: maybe_tint.copied(),
        })
    }));

    buffer.sort_unstable_by(ScreenDrawItem::cmp_draw_order);

    for item in buffer.iter() {
        match item {
            ScreenDrawItem::Panel(p) => draw_screen_panel_item(d, p, textures),
            ScreenDrawItem::Sprite(s) => draw_screen_sprite_item(d, s, textures, debug_sprites),
            ScreenDrawItem::Text(t) => draw_screen_text_item(d, t, fonts, debug_texts),
        }
    }
}

#[cfg(test)]
mod screen_draw_buffer_tests {
    use super::*;
    use crate::components::screenposition::ScreenPosition;

    fn sprite_item(z: f32) -> ScreenDrawItem {
        ScreenDrawItem::Sprite(ScreenSpriteBufferItem {
            sprite: Sprite {
                tex_key: std::sync::Arc::from("tex"),
                width: 1.0,
                height: 1.0,
                offset: Vector2::zero(),
                origin: Vector2::zero(),
                flip_h: false,
                flip_v: false,
            },
            z_index: ZIndex(z),
            pos: ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
        })
    }

    fn text_item(z: f32) -> ScreenDrawItem {
        ScreenDrawItem::Text(ScreenTextBufferItem {
            text: Arc::from("hi"),
            font: Arc::from("font"),
            font_size: 12.0,
            color: Color::WHITE,
            size: Vector2::zero(),
            z_index: ZIndex(z),
            pos: ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
        })
    }

    fn sort(mut buffer: Vec<ScreenDrawItem>) -> Vec<ScreenDrawItem> {
        buffer.sort_unstable_by(ScreenDrawItem::cmp_draw_order);
        buffer
    }

    #[test]
    fn sorts_mixed_items_by_ascending_zindex() {
        let buffer = vec![sprite_item(5.0), text_item(-2.0), sprite_item(0.0)];
        let sorted = sort(buffer);
        let zs: Vec<f32> = sorted.iter().map(|i| i.z_index().0).collect();
        assert_eq!(zs, vec![-2.0, 0.0, 5.0]);
    }

    #[test]
    fn equal_zindex_ties_break_with_text_on_top() {
        let buffer = vec![sprite_item(1.0), text_item(1.0)];
        let sorted = sort(buffer);
        assert!(matches!(sorted[0], ScreenDrawItem::Sprite(_)));
        assert!(matches!(sorted[1], ScreenDrawItem::Text(_)));
    }

    #[test]
    fn equal_zindex_tie_break_is_independent_of_insertion_order() {
        // The tie-break is encoded in `variant_rank`, not insertion order, so
        // it must hold even when texts are pushed before sprites.
        let buffer = vec![text_item(1.0), sprite_item(1.0)];
        let sorted = sort(buffer);
        assert!(matches!(sorted[0], ScreenDrawItem::Sprite(_)));
        assert!(matches!(sorted[1], ScreenDrawItem::Text(_)));
    }
}

#[cfg(test)]
mod resolve_button_patch_tests {
    use super::*;
    use std::sync::Arc;

    fn patch(tag: &str) -> GuiNinePatch {
        GuiNinePatch {
            tex_key: Arc::from(tag),
            ..GuiNinePatch::default()
        }
    }

    fn skin() -> GuiButtonSkin {
        GuiButtonSkin {
            normal: patch("normal"),
            hover: patch("hover"),
            pressed: patch("pressed"),
            disabled: patch("disabled"),
        }
    }

    #[test]
    fn resolves_each_state_to_its_matching_patch() {
        let skin = skin();
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Normal).tex_key,
            "normal"
        );
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Hovered).tex_key,
            "hover"
        );
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Pressed).tex_key,
            "pressed"
        );
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Disabled).tex_key,
            "disabled"
        );
    }
}
