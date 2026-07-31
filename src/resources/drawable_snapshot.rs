//! Render-relevant ECS state, captured once per render frame.
//!
//! [`DrawableSnapshot`] is populated by [`build_drawable_snapshot`] on the
//! `PRESENT` schedule, immediately before `render_system` reads it
//! (`src/systems/render/mod.rs`) -- captured state, not live ECS queries, and
//! no interpolation between frames (tried and reverted: it visibly lagged
//! repositioned entities, e.g. parallax backgrounds). `PRESENT` runs once,
//! after all of a tick's sim-schedule systems have completed, which is the
//! guarantee this capture needs regardless of which schedule GUI/Lua/scene
//! systems happen to run on (see `build_drawable_snapshot`'s doc comment).
//! The snapshot is published into a triple buffer and read by a separate
//! render thread.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use crate::math::Vec2;

use crate::components::boxcollider::BoxCollider;
use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::guibutton::GuiButton;
use crate::components::guiinteractable::GuiInteractable;
use crate::components::guilabel::GuiLabel;
use crate::components::guiprogressbar::GuiProgressBar;
use crate::components::guiwindow::GuiWindow;
use crate::components::mapposition::MapPosition;
use crate::components::rigidbody::RigidBody;
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::shadow::Shadow;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::protocol::stats::ThreadStats;
use crate::resources::appstate::AppState;
use crate::resources::camera2d::{Camera2D, Camera2DRes};
use crate::resources::camerafollowconfig::CameraFollowConfig;
use crate::resources::debugmode::DebugMode;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::gameconfig::GameConfig;
use crate::resources::guitheme::GuiThemeStore;
use crate::resources::input::InputState;
use crate::resources::postprocessshader::PostProcessShader;
use crate::resources::scenemanager::SceneManager;
use crate::resources::thread_stats::{AudioStats, SimStats};
use crate::resources::worldsignals::{SignalSnapshot, WorldSignals};
use crate::resources::worldtime::WorldTime;

/// Bundled to keep `build_drawable_snapshot`'s system-param count under
/// bevy_ecs's 16-param function-system limit.
#[derive(SystemParam)]
pub struct ThreadStatsParams<'w> {
    pub sim: Res<'w, SimStats>,
    pub audio: Res<'w, AudioStats>,
}

/// One world-space sprite, owned. Mirrors the fields `render_system` needs
/// for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct MapSpriteEntry {
    /// Entity id this entry was captured from. Carried through so downstream
    /// consumers (debug overlay, item lookup) don't need a live query.
    pub entity: Entity,
    pub sprite: Sprite,
    pub position: MapPosition,
    pub z_index: ZIndex,
    pub scale: Option<Scale>,
    pub rotation: Option<Rotation>,
    pub shader: Option<EntityShader>,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
    pub global_transform: Option<GlobalTransform2D>,
    /// `RigidBody.velocity`, when the entity has one -- feeds the
    /// `uVelocity` entity-shader uniform via this captured value rather than
    /// a live rigidbody query.
    pub velocity: Option<Vec2>,
}

/// One world-space text entity, owned. Mirrors `MapTextQueryData`.
#[derive(Clone, Debug, PartialEq)]
pub struct MapTextEntry {
    pub entity: Entity,
    pub text: DynamicText,
    pub position: MapPosition,
    pub z_index: ZIndex,
    pub shader: Option<EntityShader>,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
    pub global_transform: Option<GlobalTransform2D>,
    /// See [`MapSpriteEntry::velocity`].
    pub velocity: Option<Vec2>,
}

/// One screen-space sprite, owned. Mirrors `ScreenSpriteQueryData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenSpriteEntry {
    pub entity: Entity,
    pub sprite: Sprite,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
}

/// One screen-space text entity, owned. Mirrors `ScreenTextQueryData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenTextEntry {
    pub entity: Entity,
    pub text: DynamicText,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
}

/// One GUI window/panel, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct GuiWindowEntry {
    pub entity: Entity,
    pub window: GuiWindow,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI button, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct GuiButtonEntry {
    pub entity: Entity,
    pub button: GuiButton,
    pub interactable: GuiInteractable,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI label, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct GuiLabelEntry {
    pub entity: Entity,
    pub label: GuiLabel,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI progress bar, owned.
#[derive(Clone, Debug, PartialEq)]
pub struct GuiProgressBarEntry {
    pub entity: Entity,
    pub progress_bar: GuiProgressBar,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One collider box for the debug overlay, owned. `world_pos` is already
/// resolved at capture time: `GlobalTransform2D.position` when the entity has
/// one, `MapPosition.pos` otherwise (the same fallback the live query used).
#[derive(Clone, Debug)]
pub struct DebugColliderEntry {
    pub entity: Entity,
    pub collider: BoxCollider,
    pub world_pos: Vec2,
}

/// One positioned entity for the debug overlay (crosshair + optional
/// per-entity signal dump). Same `world_pos` resolution as
/// [`DebugColliderEntry`].
#[derive(Clone, Debug)]
pub struct DebugPositionEntry {
    pub entity: Entity,
    pub world_pos: Vec2,
    /// `None` when the entity has no `Signals` component, and also when
    /// `DebugOverlayConfig.show_entity_signals` is off (the clone is skipped
    /// since nothing would display it).
    pub signals: Option<Signals>,
}

/// Debug-overlay payload, captured only while `DebugMode` is active (the
/// snapshot field holding this is `None` otherwise, and consumers degrade to
/// drawing nothing / zero counts). `rigidbody_count` is a bare count because
/// that's all the overlay ever displayed of `RigidBody`.
#[derive(Clone, Debug, Default)]
pub struct DebugSnapshot {
    pub colliders: Vec<DebugColliderEntry>,
    pub positions: Vec<DebugPositionEntry>,
    pub rigidbody_count: usize,
    /// Resolved `InputState` for the F11 imgui input panel. The
    /// render thread does not resolve input itself (bindings/edges are
    /// resolved sim-side), so this is its only way to show live per-action state —
    /// debug-mode-gated like the rest of `DebugSnapshot`, zero cost when
    /// debug mode is off.
    pub input_state: InputState,
    /// Sim thread's own tick-timing rollup + input-backlog gauge, for the
    /// F11 perf panel.
    pub sim_stats: SimStats,
    /// Audio thread's tick-timing rollup, mirrored logic-side from
    /// `AudioMessage::Stats` by `land_audio_stats`.
    pub audio_stats: ThreadStats,
}

impl DebugSnapshot {
    /// Clones `self`'s contents into `dst`, reusing `dst`'s existing `Vec`
    /// backing storage for `colliders`/`positions` via `Vec::clone_from`'s
    /// capacity-reusing specialization -- mirrors
    /// `DrawableSnapshot::clone_into_buffer`, see its doc comment for why a
    /// struct-level `clone_from` (the derived one) wouldn't give this for
    /// free.
    fn clone_into_buffer(&self, dst: &mut Self) {
        dst.colliders.clone_from(&self.colliders);
        dst.positions.clone_from(&self.positions);
        dst.rigidbody_count = self.rigidbody_count;
        dst.input_state = self.input_state.clone();
        dst.sim_stats = self.sim_stats;
        dst.audio_stats = self.audio_stats;
    }
}

/// Render-relevant ECS state captured once per render frame. See the module
/// doc comment for how this is populated and transported.
///
/// Overwritten in place each render frame -- `render_system` always reads
/// whatever the latest `PRESENT`-schedule pass produced, no history
/// retention.
#[derive(Resource, Clone, Debug, Default)]
pub struct DrawableSnapshot {
    pub map_sprites: Vec<MapSpriteEntry>,
    pub map_texts: Vec<MapTextEntry>,
    pub screen_sprites: Vec<ScreenSpriteEntry>,
    pub screen_texts: Vec<ScreenTextEntry>,
    pub gui_windows: Vec<GuiWindowEntry>,
    pub gui_buttons: Vec<GuiButtonEntry>,
    pub gui_labels: Vec<GuiLabelEntry>,
    pub gui_progress_bars: Vec<GuiProgressBarEntry>,
    pub camera: Camera2D,
    /// Full copy of this frame's [`GameConfig`]. Both
    /// `render_system` and `apply_gameconfig_changes` read config exclusively
    /// from here -- this is what carries config changes across the
    /// logic->render boundary. `Default` gives real config defaults (640x360
    /// etc.), not zeros, so consumers running before the first
    /// `build_drawable_snapshot` pass can't act on a bogus 0x0 render size.
    pub game_config: GameConfig,
    /// This frame's [`WorldSignals`] snapshot. Read-only consumers
    /// on the render side (the debug world-signals panel, and the two
    /// scene callbacks' read-only `SignalSnapshot` param) use this.
    pub signals: Arc<SignalSnapshot>,
    /// Cloned [`AppState`]. `render_system`'s two scene callbacks
    /// (`GuiCallback`, `WorldDrawCallback`) read this instead of the live
    /// resource -- writes go through `SignalIntents` instead, applied
    /// logic-side by `apply_signal_intents`. `build_drawable_snapshot` skips
    /// the clone on frames where `AppState::generation()` hasn't changed.
    pub app_state: AppState,
    /// Debug-overlay payload; `Some` only while `DebugMode` is active.
    pub debug: Option<DebugSnapshot>,
    /// Active scene name. The render side resolves
    /// `gui_callback`/`world_draw_callback` against its own cloned
    /// scene-descriptor table using this key; `None` when no SceneManager is
    /// in use or no scene is active yet.
    pub active_scene: Option<Arc<str>>,
    /// This frame's [`WorldTime`]. Render-side consumers (shader
    /// time uniforms, the imgui performance panel) read this instead of the
    /// logic-world resource, preserving time_scale semantics for shader
    /// animation.
    pub world_time: WorldTime,
    /// Post-process shader chain + uniforms. Logic-owned
    /// (`GameCtx.post_process`, Lua render commands); captured on change.
    pub post_process: PostProcessShader,
    /// GUI themes. Logic-owned (`set_gui_theme_*` Lua commands
    /// re-insert the resource); captured on change — themes mutate rarely.
    pub gui_themes: GuiThemeStore,
    /// Camera-follow config copy. Display-only, for the imgui
    /// camera panel — same top-level mirror shape as `world_time`/
    /// `post_process` (copied unconditionally; the follow velocity mutates
    /// every sim tick, so change-gating would never skip anyway).
    pub camera_follow: CameraFollowConfig,
}

impl DrawableSnapshot {
    /// Clones `self`'s contents into `dst`, reusing `dst`'s existing `Vec`
    /// backing storage for the 8 drawable lists via `Vec::clone_from`'s
    /// capacity-reusing specialization.
    ///
    /// `#[derive(Clone)]` (on this struct) never generates a specialized
    /// `clone_from` -- it only generates `fn clone(&self) -> Self`, so a
    /// struct-level `dst.clone_from(self)` would fall back to the trait's
    /// default `*self = source.clone()` and allocate fresh `Vec`s every
    /// call, same cost as `dst = self.clone()`. Calling `.clone_from()`
    /// field-by-field instead gets `Vec<T>`'s own hand-written
    /// capacity-reusing `clone_from` on each of the 8 lists, which is the
    /// whole point of this method -- see `send_drawable_snapshot`
    /// (`systems/logic_bridge.rs`), the sole caller, publishing into the
    /// triple buffer's existing (stale but valid) input-side buffer instead
    /// of moving a freshly-cloned value over it.
    ///
    /// Every field must be listed by hand -- there's no compiler check tying
    /// this to the struct definition. A field added to `DrawableSnapshot`
    /// later and forgotten here doesn't fail to compile, it silently reverts
    /// to whatever stale value `dst` already held for that field.
    pub(crate) fn clone_into_buffer(&self, dst: &mut Self) {
        dst.map_sprites.clone_from(&self.map_sprites);
        dst.map_texts.clone_from(&self.map_texts);
        dst.screen_sprites.clone_from(&self.screen_sprites);
        dst.screen_texts.clone_from(&self.screen_texts);
        dst.gui_windows.clone_from(&self.gui_windows);
        dst.gui_buttons.clone_from(&self.gui_buttons);
        dst.gui_labels.clone_from(&self.gui_labels);
        dst.gui_progress_bars.clone_from(&self.gui_progress_bars);
        dst.camera = self.camera;
        dst.game_config.clone_from(&self.game_config);
        dst.signals = Arc::clone(&self.signals);
        dst.app_state.clone_from(&self.app_state);
        match (&self.debug, &mut dst.debug) {
            (Some(src), Some(existing)) => src.clone_into_buffer(existing),
            (Some(src), None) => dst.debug = Some(src.clone()),
            (None, _) => dst.debug = None,
        }
        dst.active_scene.clone_from(&self.active_scene);
        dst.world_time = self.world_time;
        dst.post_process.clone_from(&self.post_process);
        dst.gui_themes.clone_from(&self.gui_themes);
        dst.camera_follow.clone_from(&self.camera_follow);
    }
}

/// Query data shapes below mirror the fields `render_system` needs, with a
/// leading `Entity` added on each (needed so
/// every `...Entry` struct below can carry the entity id downstream, e.g.
/// for debug overlay/item lookup, without a live query).
type MapSpriteQueryData = (
    Entity,
    &'static Sprite,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static Scale>,
    Option<&'static Rotation>,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static Shadow>,
    Option<&'static GlobalTransform2D>,
    Option<&'static RigidBody>,
);

type MapTextQueryData = (
    Entity,
    &'static DynamicText,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static Shadow>,
    Option<&'static GlobalTransform2D>,
    Option<&'static RigidBody>,
);

type ScreenSpriteQueryData = (
    Entity,
    &'static Sprite,
    &'static ScreenPosition,
    &'static ZIndex,
    Option<&'static Tint>,
    Option<&'static Shadow>,
);

type ScreenTextQueryData = (
    Entity,
    &'static DynamicText,
    &'static ScreenPosition,
    &'static ZIndex,
    Option<&'static Tint>,
    Option<&'static Shadow>,
);

type GuiButtonQueryData = (
    Entity,
    &'static GuiButton,
    &'static GuiInteractable,
    &'static ScreenPosition,
    &'static ZIndex,
);

type DebugColliderQueryData = (
    Entity,
    &'static BoxCollider,
    &'static MapPosition,
    Option<&'static GlobalTransform2D>,
);

type DebugPositionQueryData = (
    Entity,
    &'static MapPosition,
    Option<&'static Signals>,
    Option<&'static GlobalTransform2D>,
);

/// Bundled read-only queries feeding [`build_drawable_snapshot`]. Covers
/// everything `render_system` draws, plus the debug-overlay queries
/// (colliders/positions/rigidbodies).
#[derive(SystemParam)]
pub struct DrawableSnapshotQueries<'w, 's> {
    map_sprites: Query<'w, 's, MapSpriteQueryData>,
    map_texts: Query<'w, 's, MapTextQueryData>,
    screen_sprites: Query<'w, 's, ScreenSpriteQueryData>,
    screen_texts: Query<'w, 's, ScreenTextQueryData>,
    gui_windows: Query<
        'w,
        's,
        (
            Entity,
            &'static GuiWindow,
            &'static ScreenPosition,
            &'static ZIndex,
        ),
    >,
    gui_buttons: Query<'w, 's, GuiButtonQueryData>,
    gui_labels: Query<
        'w,
        's,
        (
            Entity,
            &'static GuiLabel,
            &'static ScreenPosition,
            &'static ZIndex,
        ),
    >,
    gui_progress_bars: Query<
        'w,
        's,
        (
            Entity,
            &'static GuiProgressBar,
            &'static ScreenPosition,
            &'static ZIndex,
        ),
    >,
    debug_colliders: Query<'w, 's, DebugColliderQueryData>,
    debug_positions: Query<'w, 's, DebugPositionQueryData>,
    debug_rigidbodies: Query<'w, 's, (), With<RigidBody>>,
}

/// Clears `vec` and refills it from `query`, mapping each item through `f`.
/// Shared by every group in [`build_drawable_snapshot`] -- only the item
/// shape and the `f` closure vary per call site.
fn refill<D: bevy_ecs::query::ReadOnlyQueryData, T>(
    vec: &mut Vec<T>,
    query: &Query<D>,
    f: impl FnMut(D::Item<'_, '_>) -> T,
) {
    vec.clear();
    vec.extend(query.iter().map(f));
}

/// Populates [`DrawableSnapshot`] from the current ECS state. Pure
/// data-copying -- no rendering calls. Runs on the
/// `PRESENT` schedule immediately before `render_system` (see
/// `EngineBuilder::build_logic_schedules`), which by construction always runs
/// after every sim-schedule system for that tick has completed -- including
/// GUI hit-test/layout, Lua `on_update`, and scene switches, so this
/// captures fully-settled state via the schedule-ordering guarantee, not
/// because those systems share a schedule with this one. The `.after(...)`
/// edges below name what has to have settled but are vacuous cross-schedule
/// markers -- see the module doc comment.
#[allow(clippy::too_many_arguments)]
pub fn build_drawable_snapshot(
    queries: DrawableSnapshotQueries,
    camera: Res<Camera2DRes>,
    config: Res<GameConfig>,
    mut world_signals: ResMut<WorldSignals>,
    app_state: Res<AppState>,
    mut last_app_state_generation: Local<u64>,
    debug_mode: Option<Res<DebugMode>>,
    overlay_config: Res<DebugOverlayConfig>,
    world_time: Res<WorldTime>,
    post_process: Res<PostProcessShader>,
    gui_themes: Res<GuiThemeStore>,
    camera_follow: Res<CameraFollowConfig>,
    scene_manager: Option<Res<SceneManager>>,
    input_state: Res<InputState>,
    thread_stats: ThreadStatsParams,
    mut snapshot: ResMut<DrawableSnapshot>,
) {
    refill(
        &mut snapshot.map_sprites,
        &queries.map_sprites,
        |(
            entity,
            sprite,
            position,
            z_index,
            scale,
            rotation,
            shader,
            tint,
            shadow,
            global_transform,
            rigidbody,
        )| MapSpriteEntry {
            entity,
            sprite: sprite.clone(),
            position: *position,
            z_index: *z_index,
            scale: scale.copied(),
            rotation: rotation.copied(),
            shader: shader.cloned(),
            tint: tint.copied(),
            shadow: shadow.copied(),
            global_transform: global_transform.copied(),
            velocity: rigidbody.map(|rb| rb.velocity),
        },
    );

    refill(
        &mut snapshot.map_texts,
        &queries.map_texts,
        |(entity, text, position, z_index, shader, tint, shadow, global_transform, rigidbody)| {
            MapTextEntry {
                entity,
                text: text.clone(),
                position: *position,
                z_index: *z_index,
                shader: shader.cloned(),
                tint: tint.copied(),
                shadow: shadow.copied(),
                global_transform: global_transform.copied(),
                velocity: rigidbody.map(|rb| rb.velocity),
            }
        },
    );

    refill(
        &mut snapshot.screen_sprites,
        &queries.screen_sprites,
        |(entity, sprite, position, z_index, tint, shadow)| ScreenSpriteEntry {
            entity,
            sprite: sprite.clone(),
            position: *position,
            z_index: *z_index,
            tint: tint.copied(),
            shadow: shadow.copied(),
        },
    );

    refill(
        &mut snapshot.screen_texts,
        &queries.screen_texts,
        |(entity, text, position, z_index, tint, shadow)| ScreenTextEntry {
            entity,
            text: text.clone(),
            position: *position,
            z_index: *z_index,
            tint: tint.copied(),
            shadow: shadow.copied(),
        },
    );

    refill(
        &mut snapshot.gui_windows,
        &queries.gui_windows,
        |(entity, window, position, z_index)| GuiWindowEntry {
            entity,
            window: window.clone(),
            position: *position,
            z_index: *z_index,
        },
    );

    refill(
        &mut snapshot.gui_buttons,
        &queries.gui_buttons,
        |(entity, button, interactable, position, z_index)| GuiButtonEntry {
            entity,
            button: button.clone(),
            interactable: interactable.clone(),
            position: *position,
            z_index: *z_index,
        },
    );

    refill(
        &mut snapshot.gui_labels,
        &queries.gui_labels,
        |(entity, label, position, z_index)| GuiLabelEntry {
            entity,
            label: label.clone(),
            position: *position,
            z_index: *z_index,
        },
    );

    refill(
        &mut snapshot.gui_progress_bars,
        &queries.gui_progress_bars,
        |(entity, progress_bar, position, z_index)| GuiProgressBarEntry {
            entity,
            progress_bar: progress_bar.clone(),
            position: *position,
            z_index: *z_index,
        },
    );

    snapshot.camera = camera.0;
    // Config changes are rare (Lua commands); the compare short-circuits on
    // the scalar fields and skips the String/PathBuf clone on the
    // steady-state path.
    if snapshot.game_config != *config {
        snapshot.game_config = config.clone();
    }
    // Lazy Arc bump when no signal domain changed this frame; a real rebuild
    // only happens for dirty domains (see WorldSignals::snapshot).
    snapshot.signals = world_signals.snapshot();
    // Cheap generation compare skips the (potentially expensive, per-user-type) AppState
    // clone on every frame where nothing was inserted/get_mut/removed since the last capture.
    if app_state.generation() != *last_app_state_generation {
        snapshot.app_state = app_state.clone();
        *last_app_state_generation = app_state.generation();
    }
    snapshot.world_time = *world_time;
    snapshot.camera_follow = camera_follow.clone();
    // Bevy change detection gates the two rarely-mutated clones: Lua theme
    // commands re-insert GuiThemeStore (marking it changed) and post-process
    // writes go through ResMut deref_mut. is_changed() is also true on the
    // first run after insertion, so the initial capture is covered.
    if post_process.is_changed() {
        snapshot.post_process = post_process.clone();
    }
    if gui_themes.is_changed() {
        snapshot.gui_themes = gui_themes.clone();
    }
    // Arc rebuild only when the active scene actually switched.
    let active_scene = scene_manager
        .as_ref()
        .and_then(|sm| sm.active_scene.as_deref());
    if snapshot.active_scene.as_deref() != active_scene {
        snapshot.active_scene = active_scene.map(Arc::from);
    }

    if debug_mode.is_some() {
        // get_or_insert_with keeps the payload's Vec capacity alive across
        // frames while debug mode stays on -- same buffer-reuse story as
        // every other refill() in this system.
        let debug = snapshot.debug.get_or_insert_with(DebugSnapshot::default);
        refill(
            &mut debug.colliders,
            &queries.debug_colliders,
            |(entity, collider, position, maybe_gt)| DebugColliderEntry {
                entity,
                collider: *collider,
                world_pos: maybe_gt.map_or(position.pos, |gt| gt.position),
            },
        );
        // Per-entity Signals clones (3 hashmaps + 1 set each) are only paid
        // when the entity-signals overlay can actually display them.
        let capture_signals = overlay_config.show_entity_signals;
        refill(
            &mut debug.positions,
            &queries.debug_positions,
            |(entity, position, signals, maybe_gt)| DebugPositionEntry {
                entity,
                world_pos: maybe_gt.map_or(position.pos, |gt| gt.position),
                signals: if capture_signals {
                    signals.cloned()
                } else {
                    None
                },
            },
        );
        debug.rigidbody_count = queries.debug_rigidbodies.iter().count();
        debug.input_state = input_state.clone();
        debug.sim_stats = *thread_stats.sim;
        debug.audio_stats = thread_stats.audio.0;
    } else {
        snapshot.debug = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::guiinteractable::GuiWidgetState;
    use crate::components::guiprogressbar::ProgressBarDirection;
    use crate::math::Color;
    use bevy_ecs::system::RunSystemOnce;
    use crate::math::Vec2;
    use std::sync::Arc;

    fn new_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(Camera2DRes(Camera2D {
            offset: Vec2::new(0.0, 0.0),
            target: Vec2::new(1.0, 2.0),
            rotation: 0.0,
            zoom: 1.0,
        }));
        world.insert_resource(GameConfig::default());
        world.insert_resource(WorldSignals::default());
        world.insert_resource(AppState::default());
        world.insert_resource(DebugOverlayConfig::default());
        world.insert_resource(WorldTime::default());
        world.insert_resource(PostProcessShader::default());
        world.insert_resource(GuiThemeStore::default());
        world.insert_resource(CameraFollowConfig::default());
        world.insert_resource(InputState::default());
        world.insert_resource(SimStats::default());
        world.insert_resource(AudioStats::default());
        world.insert_resource(DrawableSnapshot::default());
        world
    }

    #[test]
    fn captures_map_sprite_with_optional_components() {
        let mut world = new_test_world();
        let entity = world
            .spawn((
                Sprite {
                    tex_key: Arc::from("player"),
                    width: 16.0,
                    height: 16.0,
                    offset: Vec2::new(0.0, 0.0),
                    origin: Vec2::new(0.0, 0.0),
                    flip_h: false,
                    flip_v: false,
                },
                MapPosition::new(3.0, 4.0),
                ZIndex(2.0),
                Scale {
                    scale: Vec2::new(1.0, 1.0),
                },
                Tint {
                    color: Color::WHITE,
                },
            ))
            .id();

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert_eq!(snapshot.map_sprites.len(), 1);
        let entry = &snapshot.map_sprites[0];
        assert_eq!(entry.entity, entity);
        assert_eq!(entry.sprite.tex_key.as_ref(), "player");
        assert_eq!(entry.position.pos, Vec2::new(3.0, 4.0));
        assert!(entry.scale.is_some());
        assert!(entry.tint.is_some());
        assert!(
            entry.rotation.is_none(),
            "unset optional components stay None"
        );
        assert!(entry.shadow.is_none());
    }

    #[test]
    fn captures_gui_button_with_interactable_state() {
        let mut world = new_test_world();
        world.spawn((
            GuiButton {
                size: Vec2::new(100.0, 30.0),
                caption: "Play".to_string(),
                callback_name: "on_play".into(),
                disabled: false,
                theme_key: Arc::from("default"),
            },
            GuiInteractable {
                size: Vec2::new(100.0, 30.0),
                state: GuiWidgetState::Hovered,
                on_click_callback: Some("on_play".to_string()),
                on_rust_callback: None,
            },
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert_eq!(snapshot.gui_buttons.len(), 1);
        let entry = &snapshot.gui_buttons[0];
        assert_eq!(entry.button.caption, "Play");
        assert_eq!(entry.interactable.state, GuiWidgetState::Hovered);
        assert_eq!(entry.position.pos, Vec2::new(10.0, 20.0));
    }

    #[test]
    fn captures_gui_progress_bar() {
        let mut world = new_test_world();
        world.spawn((
            GuiProgressBar {
                size: Vec2::new(50.0, 8.0),
                value: 3.0,
                max: 10.0,
                direction: ProgressBarDirection::Horizontal,
                theme_key: Arc::from("default"),
                signal_binding: None,
            },
            ScreenPosition::new(0.0, 0.0),
            ZIndex(1.0),
        ));

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert_eq!(snapshot.gui_progress_bars.len(), 1);
        assert_eq!(snapshot.gui_progress_bars[0].progress_bar.value, 3.0);
    }

    #[test]
    fn captures_camera_and_full_game_config() {
        let mut world = new_test_world();
        {
            let mut config = world.resource_mut::<GameConfig>();
            config.render_width = 320;
            config.render_height = 180;
            config.pixel_snap_camera = false;
            config.target_fps = 72;
            config.vsync = false;
            config.window_title = "phase4".to_string();
        }

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert_eq!(snapshot.game_config.render_width, 320);
        assert_eq!(snapshot.game_config.render_height, 180);
        assert!(!snapshot.game_config.pixel_snap_camera);
        assert_eq!(snapshot.game_config.target_fps, 72);
        assert_eq!(snapshot.game_config.window_title, "phase4");
        assert_eq!(snapshot.game_config, *world.resource::<GameConfig>());
        assert_eq!(snapshot.camera.target, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn captures_world_signals_snapshot() {
        let mut world = new_test_world();
        world.resource_mut::<WorldSignals>().set_flag("paused");
        world
            .resource_mut::<WorldSignals>()
            .set_integer("score", 42);

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert!(snapshot.signals.flags.contains("paused"));
        assert_eq!(snapshot.signals.integers.get("score"), Some(&42));
    }

    #[test]
    fn app_state_cloned_only_when_generation_changes() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingClone(Arc<AtomicUsize>);
        impl Clone for CountingClone {
            fn clone(&self) -> Self {
                self.0.fetch_add(1, Ordering::SeqCst);
                CountingClone(self.0.clone())
            }
        }

        // The generation-gate `Local<u64>` only persists across calls within the same
        // `Schedule` instance -- `run_system_once` builds a fresh temporary system (and
        // therefore a fresh `Local`) on every call, which would always see generation 0
        // and always reclone. A real `Schedule`, run multiple times, is what
        // `build_drawable_snapshot` actually gets in production (registered once in
        // `EngineBuilder::build_schedules`, run once per render frame).
        let mut schedule = Schedule::default();
        schedule.add_systems(build_drawable_snapshot);

        let mut world = new_test_world();
        let counter = Arc::new(AtomicUsize::new(0));
        world
            .resource_mut::<AppState>()
            .insert(CountingClone(counter.clone()));

        // First capture: generation changed from the insert above, so it must clone.
        schedule.run(&mut world);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Second capture, nothing touched AppState in between: must skip the clone.
        schedule.run(&mut world);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Mutating through get_mut bumps generation, so the next capture clones again.
        world.resource_mut::<AppState>().get_mut::<CountingClone>();
        schedule.run(&mut world);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn debug_payload_absent_without_debugmode() {
        let mut world = new_test_world();
        world.spawn((
            MapPosition::new(1.0, 1.0),
            BoxCollider::new(8.0, 8.0),
            RigidBody::default(),
        ));

        world.run_system_once(build_drawable_snapshot).unwrap();

        assert!(world.resource::<DrawableSnapshot>().debug.is_none());
    }

    #[test]
    fn debug_payload_captures_colliders_positions_rigidbodies() {
        let mut world = new_test_world();
        world.insert_resource(DebugMode {});
        let plain = world
            .spawn((
                MapPosition::new(1.0, 2.0),
                BoxCollider::new(8.0, 8.0),
                RigidBody::default(),
            ))
            .id();
        // GlobalTransform2D must win over MapPosition for world_pos.
        let child = world
            .spawn((
                MapPosition::new(5.0, 5.0),
                GlobalTransform2D {
                    position: Vec2::new(100.0, 200.0),
                    rotation_degrees: 0.0,
                    scale: Vec2::new(1.0, 1.0),
                },
                {
                    let mut signals = Signals::default();
                    signals.set_flag("on_ground");
                    signals
                },
            ))
            .id();

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        let debug = snapshot
            .debug
            .as_ref()
            .expect("payload present in debug mode");

        assert_eq!(debug.colliders.len(), 1);
        assert_eq!(debug.colliders[0].entity, plain);
        assert_eq!(debug.colliders[0].world_pos, Vec2::new(1.0, 2.0));

        assert_eq!(debug.positions.len(), 2);
        let child_entry = debug
            .positions
            .iter()
            .find(|e| e.entity == child)
            .expect("child entry present");
        assert_eq!(
            child_entry.world_pos,
            Vec2::new(100.0, 200.0),
            "GlobalTransform2D overrides MapPosition"
        );
        assert!(
            child_entry
                .signals
                .as_ref()
                .is_some_and(|s| s.get_flags().contains("on_ground"))
        );

        assert_eq!(debug.rigidbody_count, 1);
    }

    #[test]
    fn debug_payload_cleared_when_debugmode_removed() {
        let mut world = new_test_world();
        world.insert_resource(DebugMode {});
        world.run_system_once(build_drawable_snapshot).unwrap();
        assert!(world.resource::<DrawableSnapshot>().debug.is_some());

        world.remove_resource::<DebugMode>();
        world.run_system_once(build_drawable_snapshot).unwrap();
        assert!(world.resource::<DrawableSnapshot>().debug.is_none());
    }

    #[test]
    fn stale_entry_is_cleared_when_entity_despawned_between_ticks() {
        let mut world = new_test_world();
        let entity = world
            .spawn((
                Sprite {
                    tex_key: Arc::from("temp"),
                    width: 8.0,
                    height: 8.0,
                    offset: Vec2::new(0.0, 0.0),
                    origin: Vec2::new(0.0, 0.0),
                    flip_h: false,
                    flip_v: false,
                },
                MapPosition::new(0.0, 0.0),
                ZIndex(0.0),
            ))
            .id();

        world.run_system_once(build_drawable_snapshot).unwrap();
        assert_eq!(world.resource::<DrawableSnapshot>().map_sprites.len(), 1);

        world.despawn(entity);
        world.run_system_once(build_drawable_snapshot).unwrap();

        assert!(
            world.resource::<DrawableSnapshot>().map_sprites.is_empty(),
            "snapshot must not retain entries for entities despawned in a prior tick"
        );
    }
}
