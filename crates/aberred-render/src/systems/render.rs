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
//! independent of [`DebugMode`](aberred_core::resources::debug_mode::DebugMode) and is
//! intended for persistent game-developer UI
//! (HUDs, in-game editors, tool windows).

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::*;

use super::math::{
    camera2d_from_raylib, camera2d_to_raylib, color_to_raylib, vec2_from_raylib, vec2_to_raylib,
};
use crate::components::mirror::SimMirror;
use crate::resources::fontstore::FontStore;
use crate::resources::imgui_bridge::ImguiBridge;
use aberred_core::components::dynamic_text::DynamicText;
use aberred_core::components::entity_shader::EntityShader;
use aberred_core::components::gui_button::GuiButton;
use aberred_core::components::gui_interactable::{GuiInteractable, GuiWidgetState};
use aberred_core::components::gui_label::GuiLabel;
use aberred_core::components::gui_progress_bar::{GuiProgressBar, ProgressBarDirection};
use aberred_core::components::gui_window::GuiWindow;
use aberred_core::components::rotation::Rotation;
use aberred_core::components::scale::Scale;
use aberred_core::components::screen_position::ScreenPosition;
use aberred_core::components::shadow::Shadow;
use aberred_core::components::sprite::Sprite;
use aberred_core::components::tint::Tint;
use aberred_core::components::world_position::WorldPosition;
use aberred_core::components::z_index::ZIndex;
use aberred_core::math::Vec2;
use aberred_core::resources::debug_overlay_config::DebugOverlayConfig;
use aberred_core::resources::gui_theme::{
    GuiButtonSkin, GuiNinePatch, GuiThemeStore, GuiThemeWarnCache,
};
use aberred_core::resources::signal_intents::SignalIntents;

use super::mirror::MirrorQueries;
use crate::resources::mirrors::{
    RenderActiveScene, RenderAppState, RenderCamera, RenderCameraFollow, RenderDebugSnapshot,
    RenderGameConfig, RenderGuiThemes, RenderPostProcess, RenderSignalSnapshot, RenderWorldTime,
};
use crate::resources::rendertarget::RenderTarget;
use crate::resources::scene_table::RenderSceneTable;
use crate::resources::scene_table::{GuiCallback, GuiCtx};
use crate::resources::shaderstore::ShaderStore;
use crate::resources::texturestore::TextureStore;
use crate::resources::thread_stats::RenderStats;
use aberred_core::resources::render_size::RenderSize;
use aberred_core::resources::window_size::WindowSize;
use aberred_core::resources::world_time::WorldTime;
use aberred_core::systems::scene_dispatch::WorldDrawCtx;
use log::warn;

use super::debug_overlay::{PerfPanelStats, draw_imgui_debug};
use super::geometry::{compute_sprite_cull_bounds, compute_view_bounds};
use super::gui_panel::{self, draw_screen_panel_item};
use super::postprocess::apply_postprocess_passes;
use super::sprite::{draw_screen_sprite_item, draw_world_sprite_item};
use super::text::{draw_screen_text_item, draw_world_text_item};
use aberred_core::systems::sprite_geometry::resolve_world_transform;

pub(super) struct SpriteBufferItem {
    pub(super) entity: Entity,
    pub(super) sprite: Sprite,
    pub(super) z_index: ZIndex,
    pub(super) resolved_pos: WorldPosition,
    pub(super) resolved_scale: Option<Scale>,
    pub(super) resolved_rot: Option<Rotation>,
    pub(super) maybe_shader: Option<EntityShader>,
    pub(super) maybe_tint: Option<Tint>,
    pub(super) maybe_shadow: Option<Shadow>,
    pub(super) velocity: Option<Vector2>,
}

/// Per-frame read-only values shared by the world-space and screen-space draw
/// functions.
pub(super) struct DrawEnv<'a> {
    pub(super) textures: &'a TextureStore,
    pub(super) fonts: &'a FontStore,
    pub(super) world_time: &'a WorldTime,
    pub(super) render_size: &'a RenderSize,
    pub(super) window_size: &'a WindowSize,
    pub(super) show_sprite_bounds: bool,
    pub(super) show_text_bounds: bool,
}

pub(super) struct TextBufferItem {
    pub(super) entity: Entity,
    pub(super) text: DynamicText,
    pub(super) z_index: ZIndex,
    pub(super) resolved_pos: WorldPosition,
    pub(super) text_size: Vector2,
    pub(super) maybe_shader: Option<EntityShader>,
    pub(super) maybe_tint: Option<Tint>,
    pub(super) maybe_shadow: Option<Shadow>,
    pub(super) velocity: Option<Vector2>,
}

/// Screen-space sprite draw item. Simpler than [`SpriteBufferItem`]: screen-space
/// has no Scale/Rotation/GlobalTransform2D resolution, no `EntityShader` support
/// (screen-space shaders are out of scope), and no view-bounds culling.
pub(super) struct ScreenSpriteBufferItem {
    /// Sim entity, used only as `cmp_screen_draw_order`'s tie-break: mirror
    /// query iteration order carries no ordering guarantee, so this is what
    /// makes draw order deterministic. [`SpriteBufferItem::entity`] and
    /// [`TextBufferItem::entity`] tie-break the same way.
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
/// and are never read at draw time. Avoiding them keeps this struct smaller,
/// which matters for cache density when drawing tens of thousands of items per
/// frame.
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
/// sprites/text drawn on top of them (see `ScreenDrawKind::rank`).
pub(super) struct ScreenPanelBufferItem {
    /// Sim entity, used only as `cmp_screen_draw_order`'s
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

/// Which typed screen buffer a [`ScreenDrawKey`] points into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenDrawKind {
    Panel,
    ProgressBar,
    Sprite,
    Text,
}

impl ScreenDrawKind {
    /// Secondary sort key, used only to break ties at equal `z`: panel
    /// backgrounds and progress bars (0) sort below sprites (1), which sort
    /// below text (2), so a caption draws on top of its own widget's
    /// background. A progress bar is an opaque background element and appears
    /// beneath any screen-space sprite or text at the same `z`.
    fn rank(self) -> u8 {
        match self {
            ScreenDrawKind::Panel | ScreenDrawKind::ProgressBar => 0,
            ScreenDrawKind::Sprite => 1,
            ScreenDrawKind::Text => 2,
        }
    }
}

/// Small sortable handle to one screen draw item: the screen pass sorts these
/// instead of the (much larger) buffer items, then reads each item from its
/// typed buffer at `index`.
struct ScreenDrawKey {
    z: f32,
    kind: ScreenDrawKind,
    entity: Entity,
    index: u32,
}

/// Screen draw-order comparator: ascending `z`, then [`ScreenDrawKind`] rank,
/// then ascending sim entity, then buffer index. Mirror query iteration order
/// carries no ordering guarantee, so the entity tie-break (`Entity: Ord`) is
/// what makes draw order deterministic, and the index orders two items of one
/// kind that come from the same entity.
fn cmp_screen_draw_order(a: &ScreenDrawKey, b: &ScreenDrawKey) -> std::cmp::Ordering {
    // `total_cmp`, not `partial_cmp(..).unwrap_or(Equal)`: a NaN z-index would
    // otherwise break the comparator's total order, which `sort_unstable_by`
    // may panic on.
    a.z.total_cmp(&b.z)
        .then_with(|| a.kind.rank().cmp(&b.kind.rank()))
        .then_with(|| a.entity.cmp(&b.entity))
        .then_with(|| a.index.cmp(&b.index))
}

/// Rebuilds `order` as the sorted draw order of the two typed world buffers:
/// one [`WorldDrawKey`] per item, `index` addressing the item in its own
/// buffer. Reuses `order`'s capacity.
fn build_world_order(
    order: &mut Vec<WorldDrawKey>,
    sprites: &[SpriteBufferItem],
    texts: &[TextBufferItem],
) {
    order.clear();
    order.extend(sprites.iter().enumerate().map(|(i, item)| WorldDrawKey {
        z: item.z_index.0,
        kind: WorldDrawKind::Sprite,
        entity: item.entity,
        index: i as u32,
    }));
    order.extend(texts.iter().enumerate().map(|(i, item)| WorldDrawKey {
        z: item.z_index.0,
        kind: WorldDrawKind::Text,
        entity: item.entity,
        index: i as u32,
    }));
    order.sort_unstable_by(cmp_world_draw_order);
}

/// Rebuilds `order` as the sorted draw order of the four typed screen
/// buffers: one [`ScreenDrawKey`] per item, `index` addressing the item in its
/// own buffer. Reuses `order`'s capacity.
fn build_screen_order(
    order: &mut Vec<ScreenDrawKey>,
    panels: &[ScreenPanelBufferItem],
    bars: &[ScreenProgressBarBufferItem],
    sprites: &[ScreenSpriteBufferItem],
    texts: &[ScreenTextBufferItem],
) {
    order.clear();
    order.extend(panels.iter().enumerate().map(|(i, item)| ScreenDrawKey {
        z: item.z_index.0,
        kind: ScreenDrawKind::Panel,
        entity: item.entity,
        index: i as u32,
    }));
    order.extend(bars.iter().enumerate().map(|(i, item)| ScreenDrawKey {
        z: item.z_index.0,
        kind: ScreenDrawKind::ProgressBar,
        entity: item.entity,
        index: i as u32,
    }));
    order.extend(sprites.iter().enumerate().map(|(i, item)| ScreenDrawKey {
        z: item.z_index.0,
        kind: ScreenDrawKind::Sprite,
        entity: item.entity,
        index: i as u32,
    }));
    order.extend(texts.iter().enumerate().map(|(i, item)| ScreenDrawKey {
        z: item.z_index.0,
        kind: ScreenDrawKind::Text,
        entity: item.entity,
        index: i as u32,
    }));
    order.sort_unstable_by(cmp_screen_draw_order);
}

/// Which typed world buffer a [`WorldDrawKey`] points into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorldDrawKind {
    Sprite,
    Text,
}

impl WorldDrawKind {
    /// Secondary sort key, used only to break ties at equal `z`: a text (1)
    /// draws over a sprite (0), as in screen space.
    fn rank(self) -> u8 {
        match self {
            WorldDrawKind::Sprite => 0,
            WorldDrawKind::Text => 1,
        }
    }
}

/// Small sortable handle to one world draw item: the world pass sorts these
/// instead of the (much larger) buffer items, then reads each item from its
/// typed buffer at `index`.
struct WorldDrawKey {
    z: f32,
    kind: WorldDrawKind,
    entity: Entity,
    index: u32,
}

/// World draw-order comparator: ascending `z`, then [`WorldDrawKind`] rank,
/// then ascending sim entity. Mirror query iteration order carries no
/// ordering guarantee, so the entity tie-break (`Entity: Ord`, equivalent to
/// sorting by `Entity::to_bits()`) is what makes draw order deterministic:
/// items sharing an exact `z` and kind draw in ascending sim-entity order.
/// There is no index tie-break: an entity has at most one world sprite and one
/// world text.
fn cmp_world_draw_order(a: &WorldDrawKey, b: &WorldDrawKey) -> std::cmp::Ordering {
    // `total_cmp`, not `partial_cmp(..).unwrap_or(Equal)`: a NaN z-index would
    // otherwise break the comparator's total order, which `sort_unstable_by`
    // may panic on.
    a.z.total_cmp(&b.z)
        .then_with(|| a.kind.rank().cmp(&b.kind.rank()))
        .then_with(|| a.entity.cmp(&b.entity))
}

#[derive(Default)]
pub struct RenderLocals {
    sprite_buffer: Vec<SpriteBufferItem>,
    text_buffer: Vec<TextBufferItem>,
    // Sorted draw order of the two world buffers above: one key per item.
    world_order: Vec<WorldDrawKey>,
    // Scratch buffers for the screen-space items: sprites and texts built from
    // their mirror queries by render_system, panels and progress bars built
    // from the GUI mirror queries by draw_screen_space. Each frame refills
    // them (keeping their capacity, like the world buffers above) and
    // screen_order holds their sorted draw order, one key per item.
    screen_sprite_buffer: Vec<ScreenSpriteBufferItem>,
    screen_text_buffer: Vec<ScreenTextBufferItem>,
    screen_panel_buffer: Vec<ScreenPanelBufferItem>,
    screen_progress_bar_buffer: Vec<ScreenProgressBarBufferItem>,
    screen_order: Vec<ScreenDrawKey>,
}

/// Bundled render resources to reduce system parameter count.
#[derive(SystemParam)]
pub struct RenderResources<'w> {
    pub render_size: Res<'w, RenderSize>,
    pub window_size: Res<'w, WindowSize>,
    pub textures: Res<'w, TextureStore>,
    pub fonts: NonSend<'w, FontStore>,
    pub gui_theme_warn_cache: ResMut<'w, GuiThemeWarnCache>,
    // Mirrors of DrawableSnapshot's global fields, fanned out by
    // receive_snapshot (systems/snapshot.rs) -- see resources/mirrors.rs.
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
pub struct DebugResources<'w> {
    /// Buffered writes and scene/quit requests queued by `GuiCallback` --
    /// applied logic-side by `apply_signal_intents` at the top of the next
    /// sim tick.
    /// `GuiCallback`/`WorldDrawCallback` reads come from `RenderResources`
    /// (`signals`, `app_state`), not a live resource -- `render_system` holds no
    /// `ResMut<WorldSignals>`/`Res<AppState>` at all.
    pub signal_intents: ResMut<'w, SignalIntents>,
    /// Render-side scene-callback table, resolved against
    /// `RenderActiveScene` instead of the logic-world-only `SceneManager`.
    pub scene_table: Res<'w, RenderSceneTable>,
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
/// - Draws world sprites and texts in one pass sorted by `ZIndex` (see
///   `cmp_world_draw_order`); debug overlays and the scene's world-draw
///   callback follow it.
/// - When `DebugMode` is present, draws additional information (entity counts,
///   camera parameters, and optional collider boxes/signals).
/// - When the active scene's `gui_callback` is set, opens an ImGui frame and
///   calls it every frame, independent of debug mode.
///
/// Not covered by `tests/*.rs`: this system takes `NonSendMut<RenderTarget>`,
/// `NonSendMut<ImguiBridge>`, `NonSendMut<ShaderStore>`, and `NonSend<FontStore>`,
/// all of which require a live raylib/GL window context that the integration
/// test suite never starts. Its extractable pure decision logic (e.g.
/// `needs_imgui`) is unit-tested directly instead.
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
        world_order,
        screen_sprite_buffer,
        screen_text_buffer,
        screen_panel_buffer,
        screen_progress_bar_buffer,
        screen_order,
    } = &mut *locals;

    // Unpack bundled resources for easier access
    let render_size = &res.render_size;
    let window_size = &res.window_size;
    let textures = &res.textures;
    // Single source of truth for "is debug active": the debug snapshot
    // mirror's presence, captured from DebugMode by build_drawable_snapshot
    // earlier this frame. render_system takes no live Res<DebugMode> --
    // the render world doesn't have one.
    let debug_active = debug_res.debug_snapshot.0.is_some();
    let env = DrawEnv {
        textures,
        fonts,
        world_time: &res.world_time.0,
        render_size,
        window_size,
        show_sprite_bounds: debug_active && debug_res.overlay_config.show_sprite_bounds,
        show_text_bounds: debug_active && debug_res.overlay_config.show_text_bounds,
    };

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
                    aberred_core::resources::camera2d::MainCamera(camera2d_from_raylib(
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
                render_size.w as f32,
                render_size.h as f32,
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
            } // build_sprite_buffer
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
                        let resolved_pos = WorldPosition::from_vec(
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
            } // build_text_buffer
            {
                aberred_core::tracy::tracy_span!("render/sort_world");
                build_world_order(world_order, sprite_buffer, text_buffer);
            } // sort_world
            {
                aberred_core::tracy::tracy_span!("render/draw_world");
                for key in world_order.iter() {
                    let index = key.index as usize;
                    match key.kind {
                        WorldDrawKind::Sprite => draw_world_sprite_item(
                            &mut d2,
                            &sprite_buffer[index],
                            &mut shader_store,
                            &env,
                        ),
                        WorldDrawKind::Text => draw_world_text_item(
                            &mut d2,
                            &text_buffer[index],
                            &mut shader_store,
                            &env,
                        ),
                    }
                }
            } // draw_world

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
                .active_scene
                .0
                .as_deref()
                .and_then(|name| debug_res.scene_table.get(name))
                .and_then(|desc| desc.world_draw_callback)
            {
                let core_camera: aberred_core::resources::camera2d::Camera2D =
                    camera2d_from_raylib(res.camera.0);
                cb(&mut WorldDrawCtx::new(
                    &mut crate::systems::math::RaylibWorldDraw(&mut d2),
                    &core_camera,
                    &res.render_size,
                    &res.app_state.0,
                    &res.signals.0,
                ));
            }
        }

        // Draw in screen coordinates (UI layer) - still on the render target
        {
            aberred_core::tracy::tracy_span!("render/screen_space");
            // All 8 screen-space categories source from
            // retained mirror entities. Sprites and texts are built into
            // their own scratch buffers here so draw_screen_space can stay
            // agnostic to where those items came from -- it just orders and
            // draws whatever's here.
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
            // (borrowed); draw_screen_space copies only what the draw reads
            // into the typed panel and progress-bar buffers, instead of
            // materializing an owned entry per widget with all its GuiX
            // component fields (caption, tex_key, ...).
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
                &env,
                screen_panel_buffer,
                screen_progress_bar_buffer,
                screen_order,
            );
        }
    }

    // ========== PHASE 2: Multi-pass post-processing and final blit ==========
    aberred_core::tracy::tracy_span!("render/postprocess");

    // Extract gui_callback from the active scene (fn pointer is Copy — no borrow held).
    // Must be done before taking mutable borrows of other debug_res fields below.
    let gui_callback: Option<GuiCallback> = debug_res
        .active_scene
        .0
        .as_deref()
        .and_then(|name| debug_res.scene_table.get(name))
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
                render_size.w as u32,
                render_size.h as u32,
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
                        render_size,
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
                    cb(&mut GuiCtx::new(
                        ui,
                        signal_snapshot,
                        signal_intents,
                        textures,
                        fonts,
                        app_state,
                    ));
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
            &res.render_size,
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
            &res.render_size,
            &res.window_size,
            None::<fn(&RaylibDrawHandle<'_>)>,
        );
    }
}

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

/// Track and fill destination rects of a [`GuiProgressBar`] at `pos`. The
/// track always covers the whole bar; the fill covers `value / max` of it
/// (clamped to `[0, 1]`, empty when `max <= 0`), growing from the edge
/// `direction` anchors it to.
fn progress_bar_rects(
    pos: Vec2,
    size: Vec2,
    value: f32,
    max: f32,
    direction: &ProgressBarDirection,
) -> (Rectangle, Rectangle) {
    let (x, y, w, h) = (pos.x, pos.y, size.x, size.y);
    let ratio = if max > 0.0 {
        (value / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let track = Rectangle {
        x,
        y,
        width: w,
        height: h,
    };
    let fill = match direction {
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
    (track, fill)
}

fn screen_panel_item(
    entity: Entity,
    panel: GuiNinePatch,
    dest: Rectangle,
    z_index: ZIndex,
    maybe_shadow: Option<Shadow>,
) -> ScreenPanelBufferItem {
    ScreenPanelBufferItem {
        entity,
        panel,
        dest,
        z_index,
        maybe_shadow,
    }
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

/// Collects the GUI items (panels, progress bars) into their typed buffers,
/// orders those and the screen-space sprites and texts together by [`ZIndex`]
/// (see `build_screen_order`), and dispatches draw calls in that order.
///
/// Sorts small [`ScreenDrawKey`]s rather than the buffer items. The equal-z
/// tie-break (text drawn on top of a same-z sprite, the default for UI
/// captions over panel backgrounds) is part of the comparator via
/// `ScreenDrawKind::rank`.
#[allow(clippy::too_many_arguments)]
fn draw_screen_space<'m>(
    d: &mut impl RaylibDraw,
    // Already-resolved mirror-query-sourced items, read in place by index.
    screen_sprites: &[ScreenSpriteBufferItem],
    screen_texts: &[ScreenTextBufferItem],
    // Read straight off the mirror query iterators (borrowed, no per-frame
    // Vec<Entry> materialization); only the fields the draw reads are copied
    // into `panels` and `bars`.
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
    env: &DrawEnv,
    panels: &mut Vec<ScreenPanelBufferItem>,
    bars: &mut Vec<ScreenProgressBarBufferItem>,
    order: &mut Vec<ScreenDrawKey>,
) {
    panels.clear();
    bars.clear();
    for (mirror, window, p, z) in gui_windows {
        match gui_theme_store.get(&window.theme_key) {
            Some(theme) => panels.push(screen_panel_item(
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
            panels.push(screen_panel_item(
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
            panels.push(screen_panel_item(
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
        let (track_dest, fill_dest) =
            progress_bar_rects(p.pos, bar.size, bar.value, bar.max, &bar.direction);
        bars.push(ScreenProgressBarBufferItem {
            entity: mirror.0,
            track: skin.track.clone(),
            fill: skin.fill.clone(),
            track_dest,
            fill_dest,
            z_index: *z,
            maybe_shadow: theme.panel_shadow,
        });
    }

    {
        aberred_core::tracy::tracy_span!("render/sort_screen");
        build_screen_order(order, panels, bars, screen_sprites, screen_texts);
    }

    aberred_core::tracy::tracy_span!("render/draw_screen");
    for key in order.iter() {
        let index = key.index as usize;
        match key.kind {
            ScreenDrawKind::Panel => draw_screen_panel_item(d, &panels[index], env.textures),
            ScreenDrawKind::ProgressBar => {
                gui_panel::draw_screen_progress_bar_item(d, &bars[index], env.textures)
            }
            ScreenDrawKind::Sprite => draw_screen_sprite_item(
                d,
                &screen_sprites[index],
                env.textures,
                env.show_sprite_bounds,
            ),
            ScreenDrawKind::Text => {
                draw_screen_text_item(d, &screen_texts[index], env.fonts, env.show_text_bounds)
            }
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
mod resolve_button_patch_tests {
    use super::*;
    use aberred_core::math::Rect;

    fn patch(tag: &str) -> GuiNinePatch {
        GuiNinePatch::new(tag, Rect::default(), 0)
    }

    fn skin() -> GuiButtonSkin {
        GuiButtonSkin::new(patch("normal"))
            .with_hover(patch("hover"))
            .with_pressed(patch("pressed"))
            .with_disabled(patch("disabled"))
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
        let skin = GuiButtonSkin::new(patch("normal"));
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

    /// A shadow whose x offset tags where it came from.
    fn shadow(tag: f32) -> Shadow {
        Shadow::new(tag, 0.0, 0, 0, 0, 255)
    }

    fn shadow_tag(
        skin: &GuiButtonSkin,
        state: GuiWidgetState,
        theme_shadow: Option<Shadow>,
    ) -> Option<f32> {
        resolve_button_shadow(skin, state, theme_shadow).map(|s| s.offset.x)
    }

    const STATES: [GuiWidgetState; 4] = [
        GuiWidgetState::Normal,
        GuiWidgetState::Hovered,
        GuiWidgetState::Pressed,
        GuiWidgetState::Disabled,
    ];

    #[test]
    fn button_shadow_uses_the_states_own_shadow_first() {
        let skin = skin()
            .with_shadow(shadow(1.0))
            .with_hover_shadow(shadow(2.0))
            .with_pressed_shadow(shadow(3.0))
            .with_disabled_shadow(shadow(4.0));
        let tags: Vec<_> = STATES
            .iter()
            .map(|s| shadow_tag(&skin, *s, Some(shadow(9.0))))
            .collect();
        assert_eq!(tags, [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]);
    }

    #[test]
    fn button_shadow_falls_back_to_the_skin_then_the_theme_shadow() {
        let with_skin_shadow = skin().with_shadow(shadow(1.0));
        let without = skin();
        for state in STATES {
            assert_eq!(
                shadow_tag(&with_skin_shadow, state, Some(shadow(9.0))),
                Some(1.0)
            );
            assert_eq!(shadow_tag(&without, state, Some(shadow(9.0))), Some(9.0));
            assert_eq!(shadow_tag(&without, state, None), None);
        }
    }
}

#[cfg(test)]
mod progress_bar_rects_tests {
    use super::*;

    fn xywh(r: Rectangle) -> (f32, f32, f32, f32) {
        (r.x, r.y, r.width, r.height)
    }

    /// Fill rect of a 100x50 bar at (10, 20).
    fn fill(value: f32, max: f32, direction: ProgressBarDirection) -> (f32, f32, f32, f32) {
        let (_, fill) = progress_bar_rects(
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 50.0),
            value,
            max,
            &direction,
        );
        xywh(fill)
    }

    #[test]
    fn track_always_covers_the_whole_bar() {
        let (track, _) = progress_bar_rects(
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 50.0),
            0.0,
            100.0,
            &ProgressBarDirection::Vertical,
        );
        assert_eq!(xywh(track), (10.0, 20.0, 100.0, 50.0));
    }

    #[test]
    fn fill_grows_from_the_directions_anchor_edge() {
        use ProgressBarDirection::*;
        assert_eq!(fill(25.0, 100.0, Horizontal), (10.0, 20.0, 25.0, 50.0));
        assert_eq!(
            fill(25.0, 100.0, HorizontalReversed),
            (85.0, 20.0, 25.0, 50.0)
        );
        assert_eq!(fill(25.0, 100.0, Vertical), (10.0, 57.5, 100.0, 12.5));
        assert_eq!(
            fill(25.0, 100.0, VerticalReversed),
            (10.0, 20.0, 100.0, 12.5)
        );
    }

    #[test]
    fn fill_ratio_is_clamped_and_empty_without_a_positive_max() {
        use ProgressBarDirection::Horizontal;
        assert_eq!(fill(150.0, 100.0, Horizontal), (10.0, 20.0, 100.0, 50.0));
        assert_eq!(fill(-5.0, 100.0, Horizontal), (10.0, 20.0, 0.0, 50.0));
        assert_eq!(fill(5.0, 0.0, Horizontal), (10.0, 20.0, 0.0, 50.0));
        assert_eq!(fill(5.0, -1.0, Horizontal), (10.0, 20.0, 0.0, 50.0));
    }
}

#[cfg(test)]
mod world_draw_order_tests {
    use super::*;

    fn entity(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    fn key(z: f32, kind: WorldDrawKind, entity: Entity) -> WorldDrawKey {
        WorldDrawKey {
            z,
            kind,
            entity,
            index: 0,
        }
    }

    fn sprite_key(z: f32, entity: Entity) -> WorldDrawKey {
        key(z, WorldDrawKind::Sprite, entity)
    }

    fn text_key(z: f32, entity: Entity) -> WorldDrawKey {
        key(z, WorldDrawKind::Text, entity)
    }

    fn sort(mut keys: Vec<WorldDrawKey>) -> Vec<WorldDrawKey> {
        keys.sort_unstable_by(cmp_world_draw_order);
        keys
    }

    /// A NaN z-index must not break the comparator's total order (which
    /// `sort_unstable_by` may panic on); it sorts after every finite value.
    #[test]
    fn nan_z_index_sorts_last_without_panicking() {
        let mut keys: Vec<WorldDrawKey> = (0..40)
            .map(|i| {
                sprite_key(
                    if i % 3 == 0 {
                        f32::NAN
                    } else {
                        (40 - i) as f32
                    },
                    entity(i),
                )
            })
            .collect();
        keys.push(text_key(0.0, entity(99)));
        let sorted = sort(keys);
        let zs: Vec<f32> = sorted.iter().map(|k| k.z).collect();
        let first_nan = zs.iter().position(|z| z.is_nan()).unwrap();
        assert!(zs[first_nan..].iter().all(|z| z.is_nan()));
        assert!(zs[..first_nan].windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn mixed_kinds_sort_by_ascending_z_index() {
        let sorted = sort(vec![
            sprite_key(5.0, entity(0)),
            text_key(-2.0, entity(1)),
            sprite_key(0.0, entity(2)),
        ]);
        let zs: Vec<f32> = sorted.iter().map(|k| k.z).collect();
        assert_eq!(zs, vec![-2.0, 0.0, 5.0]);
    }

    /// A text below a sprite draws behind it.
    #[test]
    fn a_text_at_a_lower_z_draws_before_a_sprite_at_a_higher_z() {
        let sorted = sort(vec![sprite_key(5.0, entity(0)), text_key(1.0, entity(1))]);
        assert_eq!(sorted[0].kind, WorldDrawKind::Text);
        assert_eq!(sorted[1].kind, WorldDrawKind::Sprite);
    }

    #[test]
    fn equal_z_index_draws_the_text_after_the_sprite_whatever_the_insertion_order() {
        for keys in [
            vec![sprite_key(1.0, entity(5)), text_key(1.0, entity(1))],
            vec![text_key(1.0, entity(1)), sprite_key(1.0, entity(5))],
        ] {
            let sorted = sort(keys);
            assert_eq!(sorted[0].kind, WorldDrawKind::Sprite);
            assert_eq!(sorted[1].kind, WorldDrawKind::Text);
        }
    }

    /// `Entity`'s bit layout does not follow `from_raw_u32`'s input, so the
    /// oracle is `Entity`'s own `Ord`, not the raw index.
    #[test]
    fn equal_z_index_and_kind_tie_break_ascending_by_entity_in_any_insertion_order() {
        let source: Vec<Entity> = [42, 7, 100, 3].into_iter().map(entity).collect();
        let mut expected = source.clone();
        expected.sort();
        let orders: [[usize; 4]; 3] = [[0, 1, 2, 3], [3, 2, 1, 0], [2, 0, 3, 1]];
        for order in orders {
            let keys: Vec<WorldDrawKey> = order.iter().map(|&i| text_key(1.0, source[i])).collect();
            let ids: Vec<Entity> = sort(keys).iter().map(|k| k.entity).collect();
            assert_eq!(ids, expected);
        }
    }

    #[test]
    fn a_sprite_ranks_below_a_text() {
        assert!(WorldDrawKind::Sprite.rank() < WorldDrawKind::Text.rank());
    }

    fn sprite_item(z: f32, entity: Entity) -> SpriteBufferItem {
        SpriteBufferItem {
            entity,
            sprite: Sprite::new("tex", 1.0, 1.0),
            z_index: ZIndex(z),
            resolved_pos: WorldPosition::new(0.0, 0.0),
            resolved_scale: None,
            resolved_rot: None,
            maybe_shader: None,
            maybe_tint: None,
            maybe_shadow: None,
            velocity: None,
        }
    }

    fn text_item(z: f32, entity: Entity) -> TextBufferItem {
        TextBufferItem {
            entity,
            text: DynamicText::new("hi", "font", 12.0, aberred_core::math::Color::WHITE),
            z_index: ZIndex(z),
            resolved_pos: WorldPosition::new(0.0, 0.0),
            text_size: Vector2::zero(),
            maybe_shader: None,
            maybe_tint: None,
            maybe_shadow: None,
            velocity: None,
        }
    }

    /// The entity of the buffer item each key addresses.
    fn addressed_entities(
        order: &[WorldDrawKey],
        sprites: &[SpriteBufferItem],
        texts: &[TextBufferItem],
    ) -> Vec<Entity> {
        order
            .iter()
            .map(|k| match k.kind {
                WorldDrawKind::Sprite => sprites[k.index as usize].entity,
                WorldDrawKind::Text => texts[k.index as usize].entity,
            })
            .collect()
    }

    #[test]
    fn built_keys_address_their_own_buffer_item_in_draw_order() {
        let [e1, e2, e3, e4] = [1, 2, 3, 4].map(entity);
        let sprites = [sprite_item(2.0, e1), sprite_item(1.0, e2)];
        let texts = [text_item(1.0, e3), text_item(0.0, e4)];

        let mut order = Vec::new();
        build_world_order(&mut order, &sprites, &texts);

        // z 0 text; z 1 sprite then text (rank); z 2 sprite.
        let expected = vec![e4, e2, e3, e1];
        assert_eq!(addressed_entities(&order, &sprites, &texts), expected);
        let keyed: Vec<Entity> = order.iter().map(|k| k.entity).collect();
        assert_eq!(keyed, expected, "a key carries its item's entity");
    }

    #[test]
    fn a_key_carries_the_z_and_kind_of_its_item() {
        let mut order = Vec::new();
        build_world_order(
            &mut order,
            &[sprite_item(7.0, entity(1))],
            &[text_item(-3.0, entity(2))],
        );
        let zk: Vec<(f32, WorldDrawKind)> = order.iter().map(|k| (k.z, k.kind)).collect();
        assert_eq!(
            zk,
            vec![(-3.0, WorldDrawKind::Text), (7.0, WorldDrawKind::Sprite)]
        );
    }

    #[test]
    fn empty_buffers_give_an_empty_order() {
        let mut order = vec![key(1.0, WorldDrawKind::Text, entity(1))];
        build_world_order(&mut order, &[], &[]);
        assert!(order.is_empty());
    }

    #[test]
    fn rebuilding_replaces_the_previous_order_and_keeps_its_capacity() {
        let many: Vec<SpriteBufferItem> =
            (0..64).map(|i| sprite_item(i as f32, entity(i))).collect();
        let mut order = Vec::new();
        build_world_order(&mut order, &many, &[]);
        assert_eq!(order.len(), 64);
        let capacity = order.capacity();

        build_world_order(&mut order, &many[..3], &[]);
        assert_eq!(order.len(), 3);
        assert!(order.capacity() >= capacity);
    }
}

#[cfg(test)]
mod screen_draw_order_tests {
    use super::*;

    fn entity(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    fn key(z: f32, kind: ScreenDrawKind, entity: Entity) -> ScreenDrawKey {
        indexed_key(z, kind, entity, 0)
    }

    fn indexed_key(z: f32, kind: ScreenDrawKind, entity: Entity, index: u32) -> ScreenDrawKey {
        ScreenDrawKey {
            z,
            kind,
            entity,
            index,
        }
    }

    fn sort(mut keys: Vec<ScreenDrawKey>) -> Vec<ScreenDrawKey> {
        keys.sort_unstable_by(cmp_screen_draw_order);
        keys
    }

    fn kinds(keys: &[ScreenDrawKey]) -> Vec<ScreenDrawKind> {
        keys.iter().map(|k| k.kind).collect()
    }

    /// A NaN z-index must not break the comparator's total order (which
    /// `sort_unstable_by` may panic on); it sorts after every finite value.
    #[test]
    fn nan_z_index_sorts_last_without_panicking() {
        let mut keys: Vec<ScreenDrawKey> = (0..40)
            .map(|i| {
                key(
                    if i % 3 == 0 {
                        f32::NAN
                    } else {
                        (40 - i) as f32
                    },
                    ScreenDrawKind::Sprite,
                    entity(i),
                )
            })
            .collect();
        keys.push(key(0.0, ScreenDrawKind::Text, entity(99)));
        let sorted = sort(keys);
        let zs: Vec<f32> = sorted.iter().map(|k| k.z).collect();
        let first_nan = zs.iter().position(|z| z.is_nan()).unwrap();
        assert!(zs[first_nan..].iter().all(|z| z.is_nan()));
        assert!(zs[..first_nan].windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn mixed_kinds_sort_by_ascending_z_index() {
        let sorted = sort(vec![
            key(5.0, ScreenDrawKind::Sprite, entity(0)),
            key(-2.0, ScreenDrawKind::Text, entity(1)),
            key(0.0, ScreenDrawKind::Panel, entity(2)),
            key(3.0, ScreenDrawKind::ProgressBar, entity(3)),
        ]);
        let zs: Vec<f32> = sorted.iter().map(|k| k.z).collect();
        assert_eq!(zs, vec![-2.0, 0.0, 3.0, 5.0]);
    }

    /// At equal z a panel/progress bar is a background (rank 0), under a
    /// sprite (1), under a text (2), whatever the insertion order.
    #[test]
    fn equal_z_index_draws_backgrounds_then_sprites_then_texts() {
        let ordered = [
            (ScreenDrawKind::Panel, entity(40)),
            (ScreenDrawKind::Sprite, entity(30)),
            (ScreenDrawKind::Text, entity(20)),
        ];
        let orders: [[usize; 3]; 3] = [[0, 1, 2], [2, 1, 0], [1, 2, 0]];
        for order in orders {
            let keys = order
                .iter()
                .map(|&i| key(1.0, ordered[i].0, ordered[i].1))
                .collect();
            assert_eq!(
                kinds(&sort(keys)),
                vec![
                    ScreenDrawKind::Panel,
                    ScreenDrawKind::Sprite,
                    ScreenDrawKind::Text
                ]
            );
        }
    }

    /// Four kinds share three ranks: Panel and ProgressBar tie on rank, so they
    /// order by entity alone, not by which kind they are.
    #[test]
    fn panel_and_progress_bar_at_equal_z_tie_by_entity_not_by_kind() {
        let mut entities: Vec<Entity> = [50, 1, 20, 7].into_iter().map(entity).collect();
        entities.sort();
        // Kinds alternate along the entity order, so any comparison by kind
        // (rather than by rank) would regroup them and change the result.
        let kind_of = |i: usize| {
            if i.is_multiple_of(2) {
                ScreenDrawKind::ProgressBar
            } else {
                ScreenDrawKind::Panel
            }
        };
        let keys = [2, 0, 3, 1]
            .into_iter()
            .map(|i| key(1.0, kind_of(i), entities[i]))
            .collect();
        let ids: Vec<Entity> = sort(keys).iter().map(|k| k.entity).collect();
        assert_eq!(ids, entities);
    }

    /// `Entity`'s bit layout does not follow `from_raw_u32`'s input, so the
    /// oracle is `Entity`'s own `Ord`, not the raw index.
    #[test]
    fn equal_z_index_and_rank_tie_break_ascending_by_entity_in_any_insertion_order() {
        let source: Vec<Entity> = [42, 7, 100, 3].into_iter().map(entity).collect();
        let mut expected = source.clone();
        expected.sort();
        let orders: [[usize; 4]; 3] = [[0, 1, 2, 3], [3, 2, 1, 0], [2, 0, 3, 1]];
        for order in orders {
            let keys = order
                .iter()
                .map(|&i| key(1.0, ScreenDrawKind::Text, source[i]))
                .collect();
            let ids: Vec<Entity> = sort(keys).iter().map(|k| k.entity).collect();
            assert_eq!(ids, expected);
        }
    }

    /// One entity can emit two items of a kind; the buffer index then keeps
    /// the order deterministic.
    #[test]
    fn equal_z_index_kind_and_entity_tie_break_ascending_by_index() {
        let e = entity(5);
        for indices in [[0u32, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let keys = indices
                .iter()
                .map(|&i| indexed_key(1.0, ScreenDrawKind::Panel, e, i))
                .collect();
            let sorted: Vec<u32> = sort(keys).iter().map(|k| k.index).collect();
            assert_eq!(sorted, vec![0, 1, 2]);
        }
    }

    #[test]
    fn ranks_put_backgrounds_below_sprites_below_texts() {
        assert_eq!(
            ScreenDrawKind::Panel.rank(),
            ScreenDrawKind::ProgressBar.rank()
        );
        assert!(ScreenDrawKind::ProgressBar.rank() < ScreenDrawKind::Sprite.rank());
        assert!(ScreenDrawKind::Sprite.rank() < ScreenDrawKind::Text.rank());
    }

    fn panel(z: f32, entity: Entity) -> ScreenPanelBufferItem {
        ScreenPanelBufferItem {
            entity,
            panel: GuiNinePatch::default(),
            dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            z_index: ZIndex(z),
            maybe_shadow: None,
        }
    }

    fn bar(z: f32, entity: Entity) -> ScreenProgressBarBufferItem {
        ScreenProgressBarBufferItem {
            entity,
            track: None,
            fill: GuiNinePatch::default(),
            track_dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            fill_dest: Rectangle::new(0.0, 0.0, 1.0, 1.0),
            z_index: ZIndex(z),
            maybe_shadow: None,
        }
    }

    fn sprite(z: f32, entity: Entity) -> ScreenSpriteBufferItem {
        ScreenSpriteBufferItem {
            entity,
            sprite: Sprite::new("tex", 1.0, 1.0),
            z_index: ZIndex(z),
            pos: aberred_core::components::screen_position::ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
            maybe_shadow: None,
        }
    }

    fn text(z: f32, entity: Entity) -> ScreenTextBufferItem {
        ScreenTextBufferItem {
            entity,
            text: Arc::from("hi"),
            font: Arc::from("font"),
            font_size: 12.0,
            color: aberred_core::math::Color::WHITE,
            size: Vector2::zero(),
            z_index: ZIndex(z),
            pos: aberred_core::components::screen_position::ScreenPosition::new(0.0, 0.0),
            maybe_tint: None,
            maybe_shadow: None,
        }
    }

    /// The entity of the buffer item each key addresses.
    fn addressed_entities(
        order: &[ScreenDrawKey],
        panels: &[ScreenPanelBufferItem],
        bars: &[ScreenProgressBarBufferItem],
        sprites: &[ScreenSpriteBufferItem],
        texts: &[ScreenTextBufferItem],
    ) -> Vec<Entity> {
        order
            .iter()
            .map(|k| {
                let i = k.index as usize;
                match k.kind {
                    ScreenDrawKind::Panel => panels[i].entity,
                    ScreenDrawKind::ProgressBar => bars[i].entity,
                    ScreenDrawKind::Sprite => sprites[i].entity,
                    ScreenDrawKind::Text => texts[i].entity,
                }
            })
            .collect()
    }

    #[test]
    fn built_keys_address_their_own_buffer_item_in_draw_order() {
        let [e1, e2, e3, e4, e5, e6] = [1, 2, 3, 4, 5, 6].map(entity);
        let panels = [panel(2.0, e1), panel(1.0, e2)];
        let bars = [bar(1.0, e3)];
        let sprites = [sprite(1.0, e4), sprite(0.0, e5)];
        let texts = [text(1.0, e6)];

        let mut order = Vec::new();
        build_screen_order(&mut order, &panels, &bars, &sprites, &texts);

        // z 0 sprite; z 1: the panel and the bar (rank 0, by entity), the
        // sprite, the text; z 2 panel.
        let mut backgrounds = vec![e2, e3];
        backgrounds.sort();
        let mut expected = vec![e5];
        expected.extend(backgrounds);
        expected.extend([e4, e6, e1]);
        let addressed = addressed_entities(&order, &panels, &bars, &sprites, &texts);
        assert_eq!(addressed, expected);
        let keyed: Vec<Entity> = order.iter().map(|k| k.entity).collect();
        assert_eq!(keyed, expected, "a key carries its item's entity");
    }

    #[test]
    fn a_key_carries_the_z_and_kind_of_its_item() {
        let (panels, sprites) = ([panel(7.0, entity(1))], [sprite(-3.0, entity(2))]);
        let mut order = Vec::new();
        build_screen_order(&mut order, &panels, &[], &sprites, &[]);
        let zk: Vec<(f32, ScreenDrawKind)> = order.iter().map(|k| (k.z, k.kind)).collect();
        assert_eq!(
            zk,
            vec![(-3.0, ScreenDrawKind::Sprite), (7.0, ScreenDrawKind::Panel)]
        );
    }

    #[test]
    fn empty_buffers_give_an_empty_order() {
        let mut order = vec![key(1.0, ScreenDrawKind::Text, entity(1))];
        build_screen_order(&mut order, &[], &[], &[], &[]);
        assert!(order.is_empty());
    }

    #[test]
    fn rebuilding_replaces_the_previous_order_and_keeps_its_capacity() {
        let many: Vec<ScreenSpriteBufferItem> =
            (0..64).map(|i| sprite(i as f32, entity(i))).collect();
        let mut order = Vec::new();
        build_screen_order(&mut order, &[], &[], &many, &[]);
        assert_eq!(order.len(), 64);
        let capacity = order.capacity();

        build_screen_order(&mut order, &[], &[], &many[..3], &[]);
        assert_eq!(order.len(), 3);
        assert!(order.capacity() >= capacity);
    }
}
