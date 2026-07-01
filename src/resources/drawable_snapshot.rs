//! Render-relevant ECS state, captured once per render frame.
//!
//! Part of the Option B (render/simulation thread split) effort — see
//! `docs/render-simulation-separation-brainstorm.md`. [`DrawableSnapshot`] is
//! populated by [`build_drawable_snapshot`] on the VARIABLE schedule,
//! immediately before `render_system`; `render_system`
//! (`src/systems/render/mod.rs`) reads it directly instead of live ECS
//! queries (Phase 3) -- no interpolation between frames (tried and
//! reverted: it visibly lagged world-space entities repositioned by Lua on
//! the VARIABLE schedule, e.g. the sidescroller's parallax backgrounds,
//! since those only change once per render frame, not once per fixed
//! substep). An earlier iteration of this system captured at the end of the
//! FIXED schedule instead -- that left GUI hit-test/layout, Lua `on_update`
//! entity repositioning, and camera/config commands (all VARIABLE-schedule)
//! a full frame stale, since FIXED runs *before* VARIABLE within a frame;
//! moving the capture point to VARIABLE (right before `render_system`)
//! fixed that. Phase 5 sends the snapshot across a channel to a separate
//! render thread.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use raylib::prelude::{Camera2D, Vector2};

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
use crate::resources::camera2d::Camera2DRes;
use crate::resources::debugmode::DebugMode;
use crate::resources::debugoverlayconfig::DebugOverlayConfig;
use crate::resources::gameconfig::GameConfig;
use crate::resources::worldsignals::{SignalSnapshot, WorldSignals};

/// One world-space sprite, owned. Mirrors the fields `render_system` used to
/// query directly before Phase 3.
#[derive(Clone, Debug)]
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
    /// `uVelocity` entity-shader uniform (Phase 4 replaced `render_system`'s
    /// live rigidbody query with this captured value).
    pub velocity: Option<Vector2>,
}

/// One world-space text entity, owned. Mirrors `MapTextQueryData`.
#[derive(Clone, Debug)]
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
    pub velocity: Option<Vector2>,
}

/// One screen-space sprite, owned. Mirrors `ScreenSpriteQueryData`.
#[derive(Clone, Debug)]
pub struct ScreenSpriteEntry {
    pub entity: Entity,
    pub sprite: Sprite,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
}

/// One screen-space text entity, owned. Mirrors `ScreenTextQueryData`.
#[derive(Clone, Debug)]
pub struct ScreenTextEntry {
    pub entity: Entity,
    pub text: DynamicText,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
    pub tint: Option<Tint>,
    pub shadow: Option<Shadow>,
}

/// One GUI window/panel, owned.
#[derive(Clone, Debug)]
pub struct GuiWindowEntry {
    pub entity: Entity,
    pub window: GuiWindow,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI button, owned.
#[derive(Clone, Debug)]
pub struct GuiButtonEntry {
    pub entity: Entity,
    pub button: GuiButton,
    pub interactable: GuiInteractable,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI label, owned.
#[derive(Clone, Debug)]
pub struct GuiLabelEntry {
    pub entity: Entity,
    pub label: GuiLabel,
    pub position: ScreenPosition,
    pub z_index: ZIndex,
}

/// One GUI progress bar, owned.
#[derive(Clone, Debug)]
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
    pub world_pos: Vector2,
}

/// One positioned entity for the debug overlay (crosshair + optional
/// per-entity signal dump). Same `world_pos` resolution as
/// [`DebugColliderEntry`].
#[derive(Clone, Debug)]
pub struct DebugPositionEntry {
    pub entity: Entity,
    pub world_pos: Vector2,
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
}

/// Render-relevant ECS state captured once per render frame. See the module
/// doc comment for how this fits into the Option B plan.
///
/// Overwritten in place each render frame -- `render_system` always reads
/// whatever the latest VARIABLE-schedule pass produced, no history
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
    /// Full copy of this frame's [`GameConfig`] (Phase 4). Both
    /// `render_system` and `apply_gameconfig_changes` read config exclusively
    /// from here -- in Phase 5 this is what carries config changes across the
    /// logic->render channel. `Default` gives real config defaults (640x360
    /// etc.), not zeros, so consumers running before the first
    /// `build_drawable_snapshot` pass can't act on a bogus 0x0 render size.
    pub game_config: GameConfig,
    /// This frame's [`WorldSignals`] snapshot (Phase 4). Read-only consumers
    /// on the render side (the debug world-signals panel) use this; the two
    /// scene callbacks (`GuiCallback`, `WorldDrawCallback`) still take the
    /// live resource because they *write* intents into it -- routing those
    /// writes across threads is part of Phase 5's channel design.
    pub signals: Arc<SignalSnapshot>,
    /// Debug-overlay payload; `Some` only while `DebugMode` is active.
    pub debug: Option<DebugSnapshot>,
}

/// Query data shapes below mirror the fields `render_system` used to query
/// directly before Phase 3, with a leading `Entity` added on each (needed so
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
/// (colliders/positions/rigidbodies) that used to live in the now-deleted
/// `RenderQueries` (`src/systems/render/mod.rs`) before Phase 4.
#[derive(SystemParam)]
pub struct DrawableSnapshotQueries<'w, 's> {
    map_sprites: Query<'w, 's, MapSpriteQueryData>,
    map_texts: Query<'w, 's, MapTextQueryData>,
    screen_sprites: Query<'w, 's, ScreenSpriteQueryData>,
    screen_texts: Query<'w, 's, ScreenTextQueryData>,
    gui_windows: Query<'w, 's, (Entity, &'static GuiWindow, &'static ScreenPosition, &'static ZIndex)>,
    gui_buttons: Query<'w, 's, GuiButtonQueryData>,
    gui_labels: Query<'w, 's, (Entity, &'static GuiLabel, &'static ScreenPosition, &'static ZIndex)>,
    gui_progress_bars:
        Query<'w, 's, (Entity, &'static GuiProgressBar, &'static ScreenPosition, &'static ZIndex)>,
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
/// data-copying -- no rendering calls. Runs once per render frame, on the
/// VARIABLE schedule immediately before `render_system` (see
/// `EngineBuilder::build_schedules`), after every system that can still
/// change this frame's drawable state -- GUI hit-test/layout, Lua
/// `on_update` entity repositioning, camera/config commands, and scene
/// switches are all VARIABLE-schedule, not FIXED, so capturing here (rather
/// than at the end of the FIXED schedule, as an earlier iteration of this
/// system did) is what actually gets this frame's fully-settled state. See
/// `docs/render-simulation-separation-brainstorm.md`'s Phase 3 notes for why
/// the FIXED-tick capture point was wrong.
pub fn build_drawable_snapshot(
    queries: DrawableSnapshotQueries,
    camera: Res<Camera2DRes>,
    config: Res<GameConfig>,
    mut world_signals: ResMut<WorldSignals>,
    debug_mode: Option<Res<DebugMode>>,
    overlay_config: Res<DebugOverlayConfig>,
    mut snapshot: ResMut<DrawableSnapshot>,
) {
    refill(&mut snapshot.map_sprites, &queries.map_sprites, |(
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
    });

    refill(&mut snapshot.map_texts, &queries.map_texts, |(
        entity,
        text,
        position,
        z_index,
        shader,
        tint,
        shadow,
        global_transform,
        rigidbody,
    )| MapTextEntry {
        entity,
        text: text.clone(),
        position: *position,
        z_index: *z_index,
        shader: shader.cloned(),
        tint: tint.copied(),
        shadow: shadow.copied(),
        global_transform: global_transform.copied(),
        velocity: rigidbody.map(|rb| rb.velocity),
    });

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
                signals: if capture_signals { signals.cloned() } else { None },
            },
        );
        debug.rigidbody_count = queries.debug_rigidbodies.iter().count();
    } else {
        snapshot.debug = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::guiinteractable::GuiWidgetState;
    use crate::components::guiprogressbar::ProgressBarDirection;
    use bevy_ecs::system::RunSystemOnce;
    use raylib::prelude::{Color, Vector2};
    use std::sync::Arc;

    fn new_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(Camera2DRes(Camera2D {
            offset: Vector2::new(0.0, 0.0),
            target: Vector2::new(1.0, 2.0),
            rotation: 0.0,
            zoom: 1.0,
        }));
        world.insert_resource(GameConfig::default());
        world.insert_resource(WorldSignals::default());
        world.insert_resource(DebugOverlayConfig::default());
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
                    offset: Vector2::new(0.0, 0.0),
                    origin: Vector2::new(0.0, 0.0),
                    flip_h: false,
                    flip_v: false,
                },
                MapPosition::new(3.0, 4.0),
                ZIndex(2.0),
                Scale { scale: Vector2::new(1.0, 1.0) },
                Tint { color: Color::WHITE },
            ))
            .id();

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert_eq!(snapshot.map_sprites.len(), 1);
        let entry = &snapshot.map_sprites[0];
        assert_eq!(entry.entity, entity);
        assert_eq!(entry.sprite.tex_key.as_ref(), "player");
        assert_eq!(entry.position.pos, Vector2::new(3.0, 4.0));
        assert!(entry.scale.is_some());
        assert!(entry.tint.is_some());
        assert!(entry.rotation.is_none(), "unset optional components stay None");
        assert!(entry.shadow.is_none());
    }

    #[test]
    fn captures_gui_button_with_interactable_state() {
        let mut world = new_test_world();
        world.spawn((
            GuiButton {
                size: Vector2::new(100.0, 30.0),
                caption: "Play".to_string(),
                callback_name: "on_play".to_string(),
                disabled: false,
                theme_key: Arc::from("default"),
            },
            GuiInteractable {
                size: Vector2::new(100.0, 30.0),
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
        assert_eq!(entry.position.pos, Vector2::new(10.0, 20.0));
    }

    #[test]
    fn captures_gui_progress_bar() {
        let mut world = new_test_world();
        world.spawn((
            GuiProgressBar {
                size: Vector2::new(50.0, 8.0),
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
        assert_eq!(snapshot.camera.target, Vector2::new(1.0, 2.0));
    }

    #[test]
    fn captures_world_signals_snapshot() {
        let mut world = new_test_world();
        world.resource_mut::<WorldSignals>().set_flag("paused");
        world.resource_mut::<WorldSignals>().set_integer("score", 42);

        world.run_system_once(build_drawable_snapshot).unwrap();

        let snapshot = world.resource::<DrawableSnapshot>();
        assert!(snapshot.signals.flags.contains("paused"));
        assert_eq!(snapshot.signals.integers.get("score"), Some(&42));
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
                    position: Vector2::new(100.0, 200.0),
                    rotation_degrees: 0.0,
                    scale: Vector2::new(1.0, 1.0),
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
        let debug = snapshot.debug.as_ref().expect("payload present in debug mode");

        assert_eq!(debug.colliders.len(), 1);
        assert_eq!(debug.colliders[0].entity, plain);
        assert_eq!(debug.colliders[0].world_pos, Vector2::new(1.0, 2.0));

        assert_eq!(debug.positions.len(), 2);
        let child_entry = debug
            .positions
            .iter()
            .find(|e| e.entity == child)
            .expect("child entry present");
        assert_eq!(
            child_entry.world_pos,
            Vector2::new(100.0, 200.0),
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
                    offset: Vector2::new(0.0, 0.0),
                    origin: Vector2::new(0.0, 0.0),
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
