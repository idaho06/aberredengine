//! Rendering system using Raylib.
//!
//! Draws sprites, optional debug overlays, and basic diagnostics each frame.
//! Renders to a fixed-resolution texture, then scales to fit the window with
//! letterboxing/pillarboxing to preserve aspect ratio.
//!
//! World-space rendering uses the render-world [`RenderCamera`] mirror to
//! transform between world and screen coordinates.
//!
//! When the active scene descriptor provides a [`GuiCallback`], an ImGui frame
//! is opened every render pass and the callback is invoked. This path is
//! independent of [`DebugMode`](aberred_core::resources::debugmode::DebugMode) and is
//! intended for persistent game-developer UI
//! (HUDs, in-game editors, tool windows).

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::*;

use super::math::{camera2d_from_raylib, camera2d_to_raylib, color_to_raylib, vec2_from_raylib, vec2_to_raylib};
use aberred_core::components::dynamictext::DynamicText;
#[cfg(test)]
use aberred_core::math::Vec2;
use aberred_core::components::entityshader::EntityShader;
use aberred_core::components::guibutton::GuiButton;
use aberred_core::components::guiinteractable::{GuiInteractable, GuiWidgetState};
use aberred_core::components::guilabel::GuiLabel;
use aberred_core::components::guiprogressbar::{GuiProgressBar, ProgressBarDirection};
use aberred_core::components::guiwindow::GuiWindow;
use aberred_core::components::mapposition::MapPosition;
use crate::components::render::mirror::SimMirror;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::screenposition::ScreenPosition;
use aberred_core::components::shadow::Shadow;
use aberred_core::components::sprite::Sprite;
use aberred_core::components::tint::Tint;
use aberred_core::components::zindex::ZIndex;
use aberred_core::resources::debugoverlayconfig::DebugOverlayConfig;
use aberred_core::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiThemeStore, GuiThemeWarnCache};
use crate::resources::render::fontstore::FontStore;
use crate::resources::render::imgui_bridge::ImguiBridge;
use aberred_core::resources::signal_intents::SignalIntents;

use super::mirror::MirrorQueries;
use crate::resources::render::mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};
use crate::resources::render::rendertarget::RenderTarget;
use crate::resources::render::scene_table::RenderSceneTable;
use crate::resources::render::shaderstore::ShaderStore;
use crate::resources::render::texturestore::TextureStore;
use crate::resources::render::thread_stats::RenderStats;
use aberred_core::resources::screensize::ScreenSize;
use aberred_core::resources::windowsize::WindowSize;
use crate::resources::render::scene_table::GuiCallback;
use log::warn;

use super::debug_overlay::{PerfPanelStats, draw_imgui_debug};
use super::geometry::{
    compute_sprite_cull_bounds, compute_sprite_geometry, compute_view_bounds,
    draw_rotated_rect_lines, resolve_world_transform,
};
use super::gui_panel::{self, draw_screen_panel_item};
use super::math::{resolve_sprite_tint, resolve_text_tint, shadow_color};
use super::postprocess::{
    apply_postprocess_passes, set_entity_uniforms, set_standard_uniforms, set_uniform_value,
};
use super::sprite::draw_screen_sprite_item;
use super::text::draw_screen_text_item;

pub(super) struct SpriteBufferItem {
    entity: Entity,
    sprite: Sprite,
    z_index: ZIndex,
    resolved_pos: MapPosition,
    resolved_scale: Option<Scale>,
    resolved_rot: Option<Rotation>,
    maybe_shader: Option<EntityShader>,
    maybe_tint: Option<Tint>,
    maybe_shadow: Option<Shadow>,
    velocity: Option<Vector2>,
}

pub(super) struct TextBufferItem {
    entity: Entity,
    text: DynamicText,
    z_index: ZIndex,
    resolved_pos: MapPosition,
    text_size: Vector2,
    maybe_shader: Option<EntityShader>,
    maybe_tint: Option<Tint>,
    maybe_shadow: Option<Shadow>,
    velocity: Option<Vector2>,
}

/// Screen-space sprite draw item. Simpler than [`SpriteBufferItem`]: screen-space
/// has no Scale/Rotation/GlobalTransform2D resolution, no `EntityShader` support
/// (screen-space shaders are out of scope), and no view-bounds culling.
pub(super) struct ScreenSpriteBufferItem {
    /// Sim entity, used only as [`ScreenDrawItem::sort_key`]'s tie-break --
    /// mirror query iteration order carries no ordering guarantee, so this
    /// is what makes draw order deterministic. Same field name/type as
    /// [`SpriteBufferItem::entity`]/
    /// [`TextBufferItem::entity`], which tie-break the same way (`Entity`'s
    /// own `Ord` impl) two structs above -- no separate `u64` mechanism.
    pub(super) entity: Entity,
    pub(super) sprite: Sprite,
    pub(super) z_index: ZIndex,
    pub(super) pos: ScreenPosition,
    pub(super) maybe_tint: Option<Tint>,
    pub(super) maybe_shadow: Option<Shadow>,
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
    /// See [`ScreenSpriteBufferItem::entity`].
    pub(super) entity: Entity,
    pub(super) text: Arc<str>,
    pub(super) font: Arc<str>,
    pub(super) font_size: f32,
    pub(super) color: aberred_core::math::Color,
    pub(super) size: Vector2,
    pub(super) z_index: ZIndex,
    pub(super) pos: ScreenPosition,
    pub(super) maybe_tint: Option<Tint>,
    pub(super) maybe_shadow: Option<Shadow>,
}

/// Screen-space GUI window panel draw item. Window backgrounds sit below
/// sprites/text drawn on top of them (see [`ScreenDrawItem::variant_rank`]).
pub(super) struct ScreenPanelBufferItem {
    /// Sim entity, used only as [`ScreenDrawItem::sort_key`]'s
    /// tie-break -- see [`ScreenSpriteBufferItem::entity`].
    pub(super) entity: Entity,
    pub(super) panel: GuiNinePatch,
    pub(super) dest: Rectangle,
    pub(super) z_index: ZIndex,
    pub(super) maybe_shadow: Option<Shadow>,
}

/// Screen-space progress bar draw item. Holds both the (optional) track and
/// the fill as pre-computed `Rectangle` destinations so the dispatch loop can
/// draw track-then-fill in sequence without any intermediate sort. Using one
/// item per bar (rather than two `Panel` items) guarantees the track always
/// renders before the fill regardless of `sort_unstable_by`'s tie-breaking.
pub(super) struct ScreenProgressBarBufferItem {
    /// See [`ScreenPanelBufferItem::entity`].
    pub(super) entity: Entity,
    pub(super) track: Option<GuiNinePatch>,
    pub(super) fill: GuiNinePatch,
    pub(super) track_dest: Rectangle,
    pub(super) fill_dest: Rectangle,
    pub(super) z_index: ZIndex,
    pub(super) maybe_shadow: Option<Shadow>,
}

/// Tagged union of screen-space draw items, sorted together by [`ZIndex`] into
/// one dispatch order. Add new variants here by updating this enum plus one
/// match arm each in [`ScreenDrawItem::z_index`],
/// [`ScreenDrawItem::variant_rank`], the collect step, and the dispatch loop
/// in [`draw_screen_space`]; the sort/dispatch skeleton itself stays the
/// same.
pub(super) enum ScreenDrawItem {
    Panel(ScreenPanelBufferItem),
    ProgressBar(ScreenProgressBarBufferItem),
    Sprite(ScreenSpriteBufferItem),
    Text(ScreenTextBufferItem),
}

impl ScreenDrawItem {
    fn z_index(&self) -> ZIndex {
        match self {
            ScreenDrawItem::Panel(p) => p.z_index,
            ScreenDrawItem::ProgressBar(pb) => pb.z_index,
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
    ///
    /// `ProgressBar` shares rank 0 with `Panel`: the bar is an opaque
    /// background element and should appear beneath any screen-space sprite or
    /// text at the same `ZIndex`.
    fn variant_rank(&self) -> u8 {
        match self {
            ScreenDrawItem::Panel(_) | ScreenDrawItem::ProgressBar(_) => 0,
            ScreenDrawItem::Sprite(_) => 1,
            ScreenDrawItem::Text(_) => 2,
        }
    }

    /// Tertiary sort key (all 4 variants are mirror-sourced, each carrying
    /// its sim entity), used only to break ties at equal `(z_index,
    /// variant_rank)`: ascending sim entity. Mirror query iteration order
    /// carries no ordering guarantee, so this is what makes draw order
    /// deterministic -- including among `Panel`/`ProgressBar` items, which
    /// would otherwise have no entity to compare and tie in whatever order
    /// `sort_unstable_by` happens to produce.
    fn sort_key(&self) -> Entity {
        match self {
            ScreenDrawItem::Panel(p) => p.entity,
            ScreenDrawItem::ProgressBar(pb) => pb.entity,
            ScreenDrawItem::Sprite(s) => s.entity,
            ScreenDrawItem::Text(t) => t.entity,
        }
    }

    /// Draw-order comparator: ascending `z_index`, then `variant_rank`, then
    /// `sort_key` as tie-breaks. Shared by `draw_screen_space` and its tests
    /// so the two can't drift apart.
    fn cmp_draw_order(a: &Self, b: &Self) -> std::cmp::Ordering {
        a.z_index()
            .partial_cmp(&b.z_index())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.variant_rank().cmp(&b.variant_rank()))
            .then_with(|| a.sort_key().cmp(&b.sort_key()))
    }
}

#[derive(Default)]
pub struct RenderLocals {
    sprite_buffer: Vec<SpriteBufferItem>,
    text_buffer: Vec<TextBufferItem>,
    // Scratch buffers for the mirror-query-sourced screen-space items,
    // drained into screen_draw_buffer by draw_screen_space (rather than
    // passing the query directly, this keeps the same
    // Local-buffer-capacity-reuse convention the other 2 buffers above use).
    screen_sprite_buffer: Vec<ScreenSpriteBufferItem>,
    screen_text_buffer: Vec<ScreenTextBufferItem>,
    screen_draw_buffer: Vec<ScreenDrawItem>,
}

/// Bundled render resources to reduce system parameter count.
#[derive(SystemParam)]
pub struct RenderResources<'w> {
    pub screensize: Res<'w, ScreenSize>,
    pub window_size: Res<'w, WindowSize>,
    pub textures: Res<'w, TextureStore>,
    pub fonts: NonSend<'w, FontStore>,
    pub gui_theme_warn_cache: ResMut<'w, GuiThemeWarnCache>,
    // Mirrors of DrawableSnapshot's global fields, fanned out by
    // receive_snapshot (src/engine_app.rs) -- see src/resources/render_mirrors.rs.
    // These are render-world-only snapshot copies, not the live sim resources
    // of the same underlying type.
    pub camera: Res<'w, RenderCamera>,
    pub game_config: Res<'w, RenderGameConfig>,
    pub world_time: Res<'w, RenderWorldTime>,
    pub post_process: Res<'w, RenderPostProcess>,
    pub gui_themes: Res<'w, RenderGuiThemes>,
    pub app_state: Res<'w, RenderAppState>,
    pub signals: Res<'w, RenderSignalSnapshot>,
}

/// Extra resources needed for the imgui debug panels.
#[derive(SystemParam)]
pub(crate) struct DebugResources<'w> {
    /// Buffered writes queued by `GuiCallback` -- applied to `WorldSignals`
    /// logic-side by `apply_signal_intents` at the top of the next sim tick.
    /// `GuiCallback`/`WorldDrawCallback` reads come from `RenderResources`
    /// (`signals`, `app_state`), not a live resource -- `render_system` holds no
    /// `ResMut<WorldSignals>`/`Res<AppState>` at all.
    pub signal_intents: ResMut<'w, SignalIntents>,
    /// Render-side scene-callback table, resolved against
    /// `RenderActiveScene` instead of the logic-world-only `SceneManager`.
    pub scene_table: Option<Res<'w, RenderSceneTable>>,
    pub overlay_config: ResMut<'w, DebugOverlayConfig>,
    // active_scene kept alongside scene_table since both are
    // always read together (world_draw_callback/gui_callback dispatch);
    // debug_snapshot/camera_follow are read only inside the imgui debug
    // overlay (draw_imgui_debug, gated on debug_active).
    pub active_scene: Res<'w, RenderActiveScene>,
    pub debug_snapshot: Res<'w, RenderDebugSnapshot>,
    pub camera_follow: Res<'w, RenderCameraFollow>,
    pub render_stats: Res<'w, RenderStats>,
}

/// Tracks which render buffer is the current source during multi-pass
/// post-processing (ping-pong pattern).
#[derive(Clone, Copy)]
pub(super) enum SourceBuffer {
    Main,
    Ping,
    Pong,
}

/// Whether this frame needs an open ImGui frame at all — true when debug mode
/// is active or the active scene registered a `gui_callback`, independently.
fn needs_imgui(debug_active: bool, has_gui_callback: bool) -> bool {
    debug_active || has_gui_callback
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
///
/// Not covered by `tests/*.rs`: this system takes `NonSendMut<RenderTarget>`,
/// `NonSendMut<ImguiBridge>`, `NonSendMut<ShaderStore>`, and `NonSend<FontStore>`,
/// all of which require a live raylib/GL window context that the integration
/// test suite never starts. Its extractable pure decision logic (e.g.
/// [`needs_imgui`]) is unit-tested directly instead.
#[allow(clippy::too_many_arguments, private_interfaces)]
pub fn render_system(
    mut raylib: super::RaylibAccess,
    mut render_target: NonSendMut<RenderTarget>,
    mut imgui_bridge: NonSendMut<ImguiBridge>,
    mut shader_store: NonSendMut<ShaderStore>,
    mut res: RenderResources,
    mut debug_res: DebugResources,
    mut locals: Local<RenderLocals>,
    // Draw prep for all 8 drawable categories sources from retained mirror
    // entities instead of a DrawableSnapshot Vec -- there is no
    // render-world DrawableSnapshot resource.
    mirrors: MirrorQueries,
) {
    aberred_core::tracy::tracy_span!("render_system");
    let (rl, th) = (&mut *raylib.rl, &*raylib.th);
    let fonts = &res.fonts;
    let RenderLocals {
        sprite_buffer,
        text_buffer,
        screen_sprite_buffer,
        screen_text_buffer,
        screen_draw_buffer,
    } = &mut *locals;

    // Unpack bundled resources for easier access
    let screensize = &res.screensize;
    let window_size = &res.window_size;
    let textures = &res.textures;
    // Single source of truth for "is debug active": the debug snapshot
    // mirror's presence, captured from DebugMode by build_drawable_snapshot
    // earlier this frame. render_system takes no live Res<DebugMode> --
    // the render world doesn't have one.
    let debug_active = debug_res.debug_snapshot.0.is_some();

    // ========== PHASE 1: Render game content to the render target ==========
    {
        aberred_core::tracy::tracy_span!("render/to_texture");
        let mut d = rl.begin_texture_mode(th, &mut render_target.texture);
        let bg_color: Color = color_to_raylib(res.game_config.0.background_color);
        d.clear_background(bg_color);

        {
            // Draw in world coordinates using Camera2D.
            aberred_core::tracy::tracy_span!("render/world_space");
            let render_cam = if res.game_config.0.pixel_snap_camera {
                camera2d_to_raylib(
                    aberred_core::resources::camera2d::Camera2DRes(camera2d_from_raylib(
                        res.camera.0,
                    ))
                    .pixel_snapped(),
                )
            } else {
                res.camera.0
            };
            let mut d2 = d.begin_mode2D(render_cam);

            // `render_cam` converted to core once above and reused here across
            // all 4 corners -- `compute_view_bounds` calls this closure once
            // per corner with the same camera every time.
            let render_cam_core: aberred_core::resources::camera2d::Camera2D =
                camera2d_from_raylib(render_cam);
            let (view_min, view_max) = compute_view_bounds(
                screensize.w as f32,
                screensize.h as f32,
                render_cam,
                |pos, _cam| super::math::screen_to_world2d_raylib(pos, &render_cam_core),
            );

            {
                aberred_core::tracy::tracy_span!("render/build_sprite_buffer");
                sprite_buffer.clear();
                sprite_buffer.extend(mirrors.map_sprites.iter().filter_map(
                    |(
                        mirror,
                        sprite,
                        position,
                        z_index,
                        scale,
                        rotation,
                        shader,
                        tint,
                        shadow,
                        global_transform,
                        velocity,
                    )| {
                        let (resolved_pos, resolved_scale, resolved_rot) = resolve_world_transform(
                            *position,
                            scale.copied(),
                            rotation.copied(),
                            global_transform.copied(),
                        );
                        let (min, max) = compute_sprite_cull_bounds(
                            &resolved_pos,
                            sprite,
                            resolved_scale.as_ref(),
                            resolved_rot.as_ref(),
                        );

                        let overlap = !(max.x < view_min.x
                            || min.x > view_max.x
                            || max.y < view_min.y
                            || min.y > view_max.y);
                        overlap.then_some(SpriteBufferItem {
                            entity: mirror.0,
                            sprite: sprite.clone(),
                            z_index: *z_index,
                            resolved_pos,
                            resolved_scale,
                            resolved_rot,
                            maybe_shader: shader.cloned(),
                            maybe_tint: tint.copied(),
                            maybe_shadow: shadow.copied(),
                            velocity: velocity.map(|v| v.0),
                        })
                    },
                ));

                // Tie-break by sim entity (equivalent to sorting by
                // Entity::to_bits(), since Entity: Ord is defined that way)
                // after the primary z_index sort: mirror-entity query
                // iteration order carries no guarantee, unlike the old
                // snapshot Vec's stable (if arbitrary) build order. This
                // makes draw order deterministic again -- note for
                // reviewers: sprites sharing an exact z_index now draw in
                // ascending sim-Entity order rather than snapshot-build
                // iteration order, a minor observable ordering change.
                sprite_buffer.sort_unstable_by(|a, b| {
                    a.z_index
                        .partial_cmp(&b.z_index)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.entity.cmp(&b.entity))
                });
            } // build_sprite_buffer
            {
                aberred_core::tracy::tracy_span!("render/draw_world_sprites");
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

                        let tint_color = resolve_sprite_tint(item.maybe_tint);

                        if let Some(shadow) = item.maybe_shadow {
                            let shadow_dest = Rectangle {
                                x: dest.x + shadow.offset.x,
                                y: dest.y + shadow.offset.y,
                                ..dest
                            };
                            d2.draw_texture_pro(
                                tex,
                                src,
                                shadow_dest,
                                origin_scaled,
                                rotation,
                                shadow_color(shadow),
                            );
                        }

                        // Apply entity shader if present
                        if let Some(entity_shader) = &item.maybe_shader {
                            if let Some(entry) = shader_store.get_mut(&entity_shader.shader_key) {
                                if entry.shader.is_shader_valid() {
                                    // Set standard uniforms
                                    set_standard_uniforms(
                                        &mut entry.shader,
                                        &mut entry.locations,
                                        &res.world_time.0,
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
                                        item.velocity,
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

                        if debug_active && debug_res.overlay_config.show_sprite_bounds {
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
                aberred_core::tracy::tracy_span!("render/build_text_buffer");
                text_buffer.clear();
                text_buffer.extend(mirrors.map_texts.iter().filter_map(
                    |(
                        mirror,
                        text,
                        position,
                        z_index,
                        shader,
                        tint,
                        shadow,
                        global_transform,
                        velocity,
                    )| {
                        let resolved_pos = MapPosition::from_vec(
                            global_transform.map_or(position.pos, |gt| gt.position),
                        );
                        let text_size = text.size();
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
                            entity: mirror.0,
                            text: text.clone(),
                            z_index: *z_index,
                            resolved_pos,
                            text_size: vec2_to_raylib(text_size),
                            maybe_shader: shader.cloned(),
                            maybe_tint: tint.copied(),
                            maybe_shadow: shadow.copied(),
                            velocity: velocity.map(|v| v.0),
                        })
                    },
                ));
                // Tie-break by sim entity, same rationale as sprite_buffer's
                // sort above -- mirror query iteration order carries no
                // ordering guarantee.
                text_buffer.sort_unstable_by(|a, b| {
                    a.z_index
                        .partial_cmp(&b.z_index)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.entity.cmp(&b.entity))
                });
            } // build_text_buffer
            {
                aberred_core::tracy::tracy_span!("render/draw_world_texts");
                for item in text_buffer.iter() {
                    if let Some(font) = fonts.get(&item.text.font) {
                        let final_color = resolve_text_tint(item.maybe_tint, item.text.color);
                        let draw_pos = vec2_to_raylib(item.resolved_pos.pos);

                        if let Some(shadow) = item.maybe_shadow {
                            let shadow_pos = Vector2 {
                                x: item.resolved_pos.pos.x + shadow.offset.x,
                                y: item.resolved_pos.pos.y + shadow.offset.y,
                            };
                            d2.draw_text_ex(
                                font,
                                &item.text.text,
                                shadow_pos,
                                item.text.font_size,
                                1.0,
                                shadow_color(shadow),
                            );
                        }

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
                                        &res.world_time.0,
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
                                        item.velocity,
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
                                        draw_pos,
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
                                        draw_pos,
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
                                    draw_pos,
                                    item.text.font_size,
                                    1.0,
                                    final_color,
                                );
                            }
                        } else {
                            d2.draw_text_ex(
                                font,
                                &item.text.text,
                                draw_pos,
                                item.text.font_size,
                                1.0,
                                final_color,
                            );
                        }

                        if debug_active && debug_res.overlay_config.show_text_bounds {
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

            if let Some(debug_snapshot) = &debug_res.debug_snapshot.0 {
                if debug_res.overlay_config.show_collider_boxes {
                    for entry in &debug_snapshot.colliders {
                        let (x, y, w, h) = entry.collider.get_aabb(entry.world_pos);
                        d2.draw_rectangle_lines(x as i32, y as i32, w as i32, h as i32, Color::RED);
                    }
                }
                if debug_res.overlay_config.show_position_crosshairs
                    || debug_res.overlay_config.show_entity_signals
                {
                    for entry in &debug_snapshot.positions {
                        let world_pos = entry.world_pos;
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
                            && let Some(signals) = &entry.signals
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
                .scene_table
                .as_deref()
                .zip(debug_res.active_scene.0.as_deref())
                .and_then(|(table, name)| table.get(name))
                .and_then(|desc| desc.world_draw_callback)
            {
                let core_camera: aberred_core::resources::camera2d::Camera2D =
                    camera2d_from_raylib(res.camera.0);
                cb(
                    &mut crate::systems::render::math::RaylibWorldDraw(&mut d2),
                    &core_camera,
                    &res.screensize,
                    &res.app_state.0,
                    &res.signals.0,
                );
            }
        }

        // Draw in screen coordinates (UI layer) - still on the render target
        let debug_sprites = debug_active && debug_res.overlay_config.show_sprite_bounds;
        let debug_texts = debug_active && debug_res.overlay_config.show_text_bounds;
        {
            aberred_core::tracy::tracy_span!("render/screen_space");
            // All 8 screen-space categories source from
            // retained mirror entities. Built into their own scratch
            // buffers (not directly into screen_draw_buffer) so
            // draw_screen_space can stay agnostic to where its items came
            // from -- it just drains/iterates whatever's here.
            screen_sprite_buffer.clear();
            screen_sprite_buffer.extend(mirrors.screen_sprites.iter().map(
                |(mirror, sprite, pos, z_index, tint, shadow)| ScreenSpriteBufferItem {
                    entity: mirror.0,
                    sprite: sprite.clone(),
                    z_index: *z_index,
                    pos: *pos,
                    maybe_tint: tint.copied(),
                    maybe_shadow: shadow.copied(),
                },
            ));
            screen_text_buffer.clear();
            screen_text_buffer.extend(mirrors.screen_texts.iter().map(
                |(mirror, text, pos, z_index, tint, shadow)| ScreenTextBufferItem {
                    entity: mirror.0,
                    text: Arc::clone(&text.text),
                    font: Arc::clone(&text.font),
                    font_size: text.font_size,
                    color: text.color,
                    size: vec2_to_raylib(text.size()),
                    z_index: *z_index,
                    pos: *pos,
                    maybe_tint: tint.copied(),
                    maybe_shadow: shadow.copied(),
                },
            ));
            // GUI categories are read straight off their mirror queries
            // (borrowed, not cloned into an owned scratch buffer) --
            // unlike sprites/texts above, several GuiX component fields
            // (caption, callback_name, ...) are heap-allocated Strings that
            // a per-frame Vec<Entry> rebuild would needlessly clone.
            draw_screen_space(
                &mut d,
                screen_sprite_buffer,
                screen_text_buffer,
                mirrors.gui_windows.iter(),
                mirrors.gui_buttons.iter(),
                mirrors.gui_labels.iter(),
                mirrors.gui_progress_bars.iter(),
                &res.gui_themes.0,
                &mut res.gui_theme_warn_cache,
                textures,
                fonts,
                screen_draw_buffer,
                debug_sprites,
                debug_texts,
            );
        }
    }

    // ========== PHASE 2: Multi-pass post-processing and final blit ==========
    aberred_core::tracy::tracy_span!("render/postprocess");

    // Extract gui_callback from the active scene (fn pointer is Copy — no borrow held).
    // Must be done before taking mutable borrows of other debug_res fields below.
    let gui_callback: Option<GuiCallback> = debug_res
        .scene_table
        .as_deref()
        .zip(debug_res.active_scene.0.as_deref())
        .and_then(|(table, name)| table.get(name))
        .and_then(|desc| desc.gui_callback);

    let needs_imgui = needs_imgui(debug_active, gui_callback.is_some());

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
            perf_stats,
        ) = if let Some(debug_snapshot) = &debug_res.debug_snapshot.0 {
            let fps = rl.get_fps();
            let window_mouse_pos = rl.get_mouse_position();
            let game_mouse_pos = vec2_to_raylib(window_size.window_to_game_pos(
                vec2_from_raylib(window_mouse_pos),
                screensize.w as u32,
                screensize.h as u32,
            ));
            let mouse_world = super::math::screen_to_world2d_raylib(
                game_mouse_pos,
                &camera2d_from_raylib(res.camera.0),
            );
            // Query::count() (not .iter().count()) takes the optimized path for
            // archetypal queries -- table/archetype-count arithmetic instead of
            // walking every matched entity, closer to the old Vec::len() cost.
            let sprite_count = mirrors.map_sprites.count();
            let collider_count = debug_snapshot.colliders.len();
            let position_count = debug_snapshot.positions.len();
            let rigidbody_count = debug_snapshot.rigidbody_count;
            let screen_sprite_count = mirrors.screen_sprites.count();
            let screen_text_count = mirrors.screen_texts.count();
            let shader_count = shader_store.len();
            let perf_stats = PerfPanelStats {
                sim: debug_snapshot.sim_stats,
                audio: debug_snapshot.audio_stats,
                render: debug_res.render_stats.0,
            };
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
                perf_stats,
            )
        } else {
            // Dummy values — only reached when gui_callback is Some; debug_active is false
            // so the debug branch inside the closure will not execute them.
            (
                0,
                Vector2::zero(),
                Vector2::zero(),
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                PerfPanelStats {
                    sim: Default::default(),
                    audio: Default::default(),
                    render: Default::default(),
                },
            )
        };

        // Extract refs before closure (avoids borrow conflict with apply_postprocess_passes)
        let overlay_config = &mut *debug_res.overlay_config;
        let signal_intents = &mut *debug_res.signal_intents;
        let app_state = &res.app_state.0;
        // Only `Some` while DebugMode is active (`debug_active`); the
        // closure below only reads it inside `if debug_active`, mirroring
        // the dummy-values pattern used for fps/sprite_count/etc. above.
        let input_state = debug_res.debug_snapshot.0.as_ref().map(|d| &d.input_state);
        let camera_follow = &debug_res.camera_follow.0;
        let active_scene = debug_res.active_scene.0.as_deref();
        let world_time = &res.world_time.0;
        let config = &res.game_config.0;
        let signal_snapshot = &*res.signals.0;
        let camera = &res.camera.0;

        let closure = move |_d: &RaylibDrawHandle<'_>| {
            imgui_bridge.render(debug_active, |ui| {
                if debug_active {
                    draw_imgui_debug(
                        ui,
                        overlay_config,
                        signal_snapshot,
                        input_state.expect("debug_active implies debug_snapshot is Some"),
                        camera,
                        camera_follow,
                        active_scene,
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
                        &perf_stats,
                    );
                }

                if let Some(cb) = gui_callback {
                    cb(
                        ui,
                        signal_snapshot,
                        signal_intents,
                        textures,
                        fonts,
                        app_state,
                    );
                }
            });
        };
        apply_postprocess_passes(
            rl,
            th,
            &mut render_target,
            &mut shader_store,
            &res.post_process.0,
            world_time,
            &res.screensize,
            &res.window_size,
            Some(closure),
        );
    } else {
        // needs_imgui was false this frame, so `render()` (and therefore the
        // capture snapshot it takes) doesn't run -- clear explicitly so
        // capture flags don't freeze at their last computed value once the
        // debug overlay closes.
        imgui_bridge.clear_capture();
        apply_postprocess_passes(
            rl,
            th,
            &mut render_target,
            &mut shader_store,
            &res.post_process.0,
            &res.world_time.0,
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
/// Selects the nine-patch for a `GuiButton`'s current state from its skin,
/// falling back to `normal` for any state whose patch was never set.
fn resolve_button_patch(skin: &GuiButtonSkin, state: GuiWidgetState) -> &GuiNinePatch {
    match state {
        GuiWidgetState::Normal => &skin.normal,
        GuiWidgetState::Hovered => skin.hover.as_ref().unwrap_or(&skin.normal),
        GuiWidgetState::Pressed => skin.pressed.as_ref().unwrap_or(&skin.normal),
        GuiWidgetState::Disabled => skin.disabled.as_ref().unwrap_or(&skin.normal),
    }
}

fn resolve_button_shadow(
    skin: &GuiButtonSkin,
    state: GuiWidgetState,
    theme_shadow: Option<Shadow>,
) -> Option<Shadow> {
    let skin_shadow = match state {
        GuiWidgetState::Normal => skin.shadow,
        GuiWidgetState::Hovered => skin.hover_shadow.or(skin.shadow),
        GuiWidgetState::Pressed => skin.pressed_shadow.or(skin.shadow),
        GuiWidgetState::Disabled => skin.disabled_shadow.or(skin.shadow),
    };
    skin_shadow.or(theme_shadow)
}

fn screen_panel_item(
    entity: Entity,
    panel: GuiNinePatch,
    dest: Rectangle,
    z_index: ZIndex,
    maybe_shadow: Option<Shadow>,
) -> ScreenDrawItem {
    ScreenDrawItem::Panel(ScreenPanelBufferItem {
        entity,
        panel,
        dest,
        z_index,
        maybe_shadow,
    })
}

fn warn_missing_theme(
    gui_theme_warn_cache: &mut GuiThemeWarnCache,
    widget_kind: &str,
    theme_key: &str,
    detail: &str,
) {
    if gui_theme_warn_cache.warn_once(theme_key) {
        warn!("{widget_kind} references unregistered theme_key '{theme_key}'{detail}");
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_screen_space<'m>(
    d: &mut impl RaylibDraw,
    // Already-resolved mirror-query-sourced items, drained (not iterated)
    // into `buffer` -- draining reuses each item's capacity for the
    // caller's next-frame scratch buffer with zero clone, matching the
    // Local-buffer-reuse convention `sprite_buffer`/`text_buffer` already
    // use above.
    screen_sprites: &mut Vec<ScreenSpriteBufferItem>,
    screen_texts: &mut Vec<ScreenTextBufferItem>,
    // Read straight off the mirror query iterators (borrowed, no per-frame
    // Vec<Entry> materialization) -- several GuiX component fields
    // (caption, callback_name, ...) are heap-allocated Strings that an
    // owned scratch-buffer rebuild would needlessly clone every frame.
    gui_windows: impl Iterator<Item = (&'m SimMirror, &'m GuiWindow, &'m ScreenPosition, &'m ZIndex)>,
    gui_buttons: impl Iterator<
        Item = (
            &'m SimMirror,
            &'m GuiButton,
            &'m GuiInteractable,
            &'m ScreenPosition,
            &'m ZIndex,
        ),
    >,
    gui_labels: impl Iterator<Item = (&'m SimMirror, &'m GuiLabel, &'m ScreenPosition, &'m ZIndex)>,
    gui_progress_bars: impl Iterator<
        Item = (
            &'m SimMirror,
            &'m GuiProgressBar,
            &'m ScreenPosition,
            &'m ZIndex,
        ),
    >,
    gui_theme_store: &GuiThemeStore,
    gui_theme_warn_cache: &mut GuiThemeWarnCache,
    textures: &TextureStore,
    fonts: &FontStore,
    buffer: &mut Vec<ScreenDrawItem>,
    debug_sprites: bool,
    debug_texts: bool,
) {
    buffer.clear();
    for (mirror, window, p, z) in gui_windows {
        match gui_theme_store.get(&window.theme_key) {
            Some(theme) => buffer.push(screen_panel_item(
                mirror.0,
                theme.panel.clone(),
                Rectangle {
                    x: p.pos.x,
                    y: p.pos.y,
                    width: window.size.x,
                    height: window.size.y,
                },
                *z,
                theme.panel_shadow,
            )),
            None => warn_missing_theme(
                gui_theme_warn_cache,
                "GuiWindow",
                &window.theme_key,
                " — skipping themed background",
            ),
        }
    }
    for (mirror, button, interactable, p, z) in gui_buttons {
        let Some(theme) = gui_theme_store.get(&button.theme_key) else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiButton",
                &button.theme_key,
                " — skipping themed background",
            );
            continue;
        };
        if let Some(skin) = theme.button.as_ref() {
            buffer.push(screen_panel_item(
                mirror.0,
                resolve_button_patch(skin, interactable.state).clone(),
                Rectangle {
                    x: p.pos.x,
                    y: p.pos.y,
                    width: interactable.size.x,
                    height: interactable.size.y,
                },
                *z,
                resolve_button_shadow(skin, interactable.state, theme.panel_shadow),
            ));
        } else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiButton",
                &button.theme_key,
                " has no button skin — skipping themed background",
            );
        }
    }
    for (mirror, label, p, z) in gui_labels {
        let Some(theme) = gui_theme_store.get(&label.theme_key) else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiLabel",
                &label.theme_key,
                " — skipping themed background",
            );
            continue;
        };
        if let Some(patch) = theme.label.as_ref() {
            buffer.push(screen_panel_item(
                mirror.0,
                patch.clone(),
                Rectangle {
                    x: p.pos.x,
                    y: p.pos.y,
                    width: label.size.x,
                    height: label.size.y,
                },
                *z,
                theme.panel_shadow,
            ));
        } else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiLabel",
                &label.theme_key,
                " has no label patch — skipping themed background",
            );
        }
    }
    for (mirror, bar, p, z) in gui_progress_bars {
        let Some(theme) = gui_theme_store.get(&bar.theme_key) else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiProgressBar",
                &bar.theme_key,
                " (or that theme has no progress_bar skin) — skipping bar",
            );
            continue;
        };
        let Some(skin) = theme.progress_bar.as_ref() else {
            warn_missing_theme(
                gui_theme_warn_cache,
                "GuiProgressBar",
                &bar.theme_key,
                " (or that theme has no progress_bar skin) — skipping bar",
            );
            continue;
        };
        let x = p.pos.x;
        let y = p.pos.y;
        let w = bar.size.x;
        let h = bar.size.y;
        let ratio = if bar.max > 0.0 {
            (bar.value / bar.max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let track_dest = Rectangle {
            x,
            y,
            width: w,
            height: h,
        };
        let fill_dest = match bar.direction {
            ProgressBarDirection::Horizontal => Rectangle {
                x,
                y,
                width: w * ratio,
                height: h,
            },
            ProgressBarDirection::HorizontalReversed => {
                let fill_w = w * ratio;
                Rectangle {
                    x: x + w - fill_w,
                    y,
                    width: fill_w,
                    height: h,
                }
            }
            ProgressBarDirection::Vertical => {
                let fill_h = h * ratio;
                Rectangle {
                    x,
                    y: y + h - fill_h,
                    width: w,
                    height: fill_h,
                }
            }
            ProgressBarDirection::VerticalReversed => Rectangle {
                x,
                y,
                width: w,
                height: h * ratio,
            },
        };
        buffer.push(ScreenDrawItem::ProgressBar(ScreenProgressBarBufferItem {
            entity: mirror.0,
            track: skin.track.clone(),
            fill: skin.fill.clone(),
            track_dest,
            fill_dest,
            z_index: *z,
            maybe_shadow: theme.panel_shadow,
        }));
    }
    buffer.extend(screen_sprites.drain(..).map(ScreenDrawItem::Sprite));
    buffer.extend(screen_texts.drain(..).map(ScreenDrawItem::Text));

    buffer.sort_unstable_by(ScreenDrawItem::cmp_draw_order);

    for item in buffer.iter() {
        match item {
            ScreenDrawItem::Panel(p) => draw_screen_panel_item(d, p, textures),
            ScreenDrawItem::ProgressBar(pb) => {
                gui_panel::draw_screen_progress_bar_item(d, pb, textures)
            }
            ScreenDrawItem::Sprite(s) => draw_screen_sprite_item(d, s, textures, debug_sprites),
            ScreenDrawItem::Text(t) => draw_screen_text_item(d, t, fonts, debug_texts),
        }
    }
}

#[cfg(test)]
mod needs_imgui_tests {
    use super::needs_imgui;

    #[test]
    fn neither_debug_nor_gui_callback_skips_imgui() {
        assert!(!needs_imgui(false, false));
    }

    #[test]
    fn debug_active_alone_needs_imgui() {
        assert!(needs_imgui(true, false));
    }

    #[test]
    fn gui_callback_alone_needs_imgui() {
        assert!(needs_imgui(false, true));
    }

    #[test]
    fn both_debug_and_gui_callback_needs_imgui() {
        assert!(needs_imgui(true, true));
    }
}

#[cfg(test)]
mod screen_draw_buffer_tests {
    use super::*;
    use aberred_core::components::screenposition::ScreenPosition;

    fn sprite_item(z: f32) -> ScreenDrawItem {
        sprite_item_with_entity(z, Entity::from_raw_u32(0).unwrap())
    }

    fn sprite_item_with_entity(z: f32, entity: Entity) -> ScreenDrawItem {
        ScreenDrawItem::Sprite(ScreenSpriteBufferItem {
            entity,
            sprite: Sprite {
                tex_key: std::sync::Arc::from("tex"),
                width: 1.0,
                height: 1.0,
                offset: Vec2::ZERO,
                origin: Vec2::ZERO,
                flip_h: false,
                flip_v: false,
            },
            z_index: ZIndex(z),
            pos: ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
            maybe_shadow: None,
        })
    }

    fn text_item(z: f32) -> ScreenDrawItem {
        text_item_with_entity(z, Entity::from_raw_u32(0).unwrap())
    }

    fn text_item_with_entity(z: f32, entity: Entity) -> ScreenDrawItem {
        ScreenDrawItem::Text(ScreenTextBufferItem {
            entity,
            text: Arc::from("hi"),
            font: Arc::from("font"),
            font_size: 12.0,
            color: aberred_core::math::Color::WHITE,
            size: Vector2::zero(),
            z_index: ZIndex(z),
            pos: ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
            maybe_shadow: None,
        })
    }

    fn panel_item_with_entity(z: f32, entity: Entity) -> ScreenDrawItem {
        ScreenDrawItem::Panel(ScreenPanelBufferItem {
            entity,
            panel: GuiNinePatch::default(),
            dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            z_index: ZIndex(z),
            maybe_shadow: None,
        })
    }

    fn progress_bar_item_with_entity(z: f32, entity: Entity) -> ScreenDrawItem {
        ScreenDrawItem::ProgressBar(ScreenProgressBarBufferItem {
            entity,
            track: None,
            fill: GuiNinePatch::default(),
            track_dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            fill_dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            z_index: ZIndex(z),
            maybe_shadow: None,
        })
    }

    fn sort(mut buffer: Vec<ScreenDrawItem>) -> Vec<ScreenDrawItem> {
        buffer.sort_unstable_by(ScreenDrawItem::cmp_draw_order);
        buffer
    }

    /// All 4 `ScreenDrawItem` variants are mirror-sourced (GUI
    /// windows/buttons/labels all become `Panel` items, progress bars their
    /// own variant), so ties at equal `(z_index, variant_rank)` must resolve
    /// deterministically via `sort_key()`'s `Entity` ordering across EVERY
    /// variant. `Panel`/`ProgressBar` share `variant_rank` 0, so this
    /// specifically exercises their tie-break against each other.
    #[test]
    fn all_four_variants_tie_break_ascending_by_entity_at_equal_rank() {
        let mut entities: Vec<Entity> = [50, 1, 20]
            .into_iter()
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect();
        // Panel and ProgressBar share variant_rank 0 -- build one of each
        // sharing this entity set so their relative order is exercised.
        let buffer: Vec<ScreenDrawItem> = vec![
            panel_item_with_entity(1.0, entities[0]),
            progress_bar_item_with_entity(1.0, entities[1]),
            panel_item_with_entity(1.0, entities[2]),
        ];
        let sorted = sort(buffer);
        let ids: Vec<Entity> = sorted.iter().map(ScreenDrawItem::sort_key).collect();

        entities.sort();
        assert_eq!(
            ids, entities,
            "Panel/ProgressBar ties must now resolve by ascending Entity"
        );
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

    #[test]
    fn equal_zindex_and_variant_ties_break_ascending_by_entity() {
        // Two sprites at the same z_index (so variant_rank ties too) must
        // order deterministically by Entity's own Ord impl, since mirror
        // query iteration order carries no ordering guarantee.
        // Compares against `entities.sort()` rather than a hand-guessed
        // order: Entity's internal bit layout (NonMaxU32-niched index) does
        // NOT correlate monotonically with `from_raw_u32`'s input, so the
        // only correct oracle for "ascending by Entity" is `Entity`'s own
        // `Ord` impl, not raw index magnitude.
        let mut entities: Vec<Entity> = [42, 7, 100]
            .into_iter()
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect();
        let buffer: Vec<ScreenDrawItem> = entities
            .iter()
            .map(|&e| sprite_item_with_entity(1.0, e))
            .collect();
        let sorted = sort(buffer);
        let ids: Vec<Entity> = sorted.iter().map(ScreenDrawItem::sort_key).collect();

        entities.sort();
        assert_eq!(ids, entities);
    }

    #[test]
    fn equal_zindex_and_variant_ties_break_ascending_by_entity_for_text() {
        let mut entities: Vec<Entity> = [9, 3]
            .into_iter()
            .map(|i| Entity::from_raw_u32(i).unwrap())
            .collect();
        let buffer: Vec<ScreenDrawItem> = entities
            .iter()
            .map(|&e| text_item_with_entity(1.0, e))
            .collect();
        let sorted = sort(buffer);
        let ids: Vec<Entity> = sorted.iter().map(ScreenDrawItem::sort_key).collect();

        entities.sort();
        assert_eq!(ids, entities);
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
            hover: Some(patch("hover")),
            pressed: Some(patch("pressed")),
            disabled: Some(patch("disabled")),
            ..GuiButtonSkin::default()
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

    #[test]
    fn falls_back_to_normal_when_state_patch_unset() {
        let skin = GuiButtonSkin {
            normal: patch("normal"),
            ..GuiButtonSkin::default()
        };
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Hovered).tex_key,
            "normal"
        );
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Pressed).tex_key,
            "normal"
        );
        assert_eq!(
            &*resolve_button_patch(&skin, GuiWidgetState::Disabled).tex_key,
            "normal"
        );
    }
}
