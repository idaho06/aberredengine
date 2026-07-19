//! Retained render-world "mirror" entities: one bevy_ecs entity per
//! drawable-list item, keyed by the sim entity's `Entity::to_bits()`,
//! reconciled every time a new `DrawableSnapshot` arrives instead of rebuilt
//! from a `Vec` every frame.
//!
//! All 8 `DrawableSnapshot` drawable categories -- map sprites, map texts,
//! screen sprites, screen texts, and the 4 GUI categories (windows, buttons,
//! labels, progress bars) -- are mirrored this way. The render world holds
//! no `DrawableSnapshot` *resource* itself; the type survives only as the
//! sim thread's wire format, see `src/protocol/snapshot.rs`.
//!
//! Write-only from the snapshot: reconciliation must never read or mutate
//! anything that would make the render world start carrying gameplay logic
//! -- that's the architectural line this module defends.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::components::dynamictext::DynamicText;
use crate::components::entityshader::EntityShader;
use crate::components::globaltransform2d::GlobalTransform2D;
use crate::components::guibutton::GuiButton;
use crate::components::guiinteractable::GuiInteractable;
use crate::components::guilabel::GuiLabel;
use crate::components::guiprogressbar::GuiProgressBar;
use crate::components::guiwindow::GuiWindow;
use crate::components::mapposition::MapPosition;
use crate::components::render::mirror::{
    MirrorGuiButton, MirrorGuiLabel, MirrorGuiProgressBar, MirrorGuiWindow, MirrorMapSprite,
    MirrorMapText, MirrorScreenSprite, MirrorScreenText, MirrorVelocity, SimMirror,
};
use crate::components::rotation::Rotation;
use crate::components::scale::Scale;
use crate::components::screenposition::ScreenPosition;
use crate::components::shadow::Shadow;
use crate::components::sprite::Sprite;
use crate::components::tint::Tint;
use crate::components::zindex::ZIndex;
use crate::resources::drawable_snapshot::{
    GuiButtonEntry, GuiLabelEntry, GuiProgressBarEntry, GuiWindowEntry, MapSpriteEntry,
    MapTextEntry, ScreenSpriteEntry, ScreenTextEntry,
};
use crate::resources::render::sim_id_map::SimIdMap;

/// Implemented by every `DrawableSnapshot` entry type [`reconcile`] accepts,
/// so the shared loop can extract the originating sim `Entity` without a
/// per-call closure -- every entry type's `entity` field access is identical,
/// so a closure parameter here would only add indirection without adding
/// flexibility.
trait SimEntry {
    fn sim_entity(&self) -> Entity;
}

impl SimEntry for MapSpriteEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for MapTextEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for ScreenSpriteEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for ScreenTextEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for GuiWindowEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for GuiButtonEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for GuiLabelEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}
impl SimEntry for GuiProgressBarEntry {
    fn sim_entity(&self) -> Entity {
        self.entity
    }
}

/// Insert a clone of `value` only if the entity's current component value
/// differs from it (or the component is absent). Every publish (see
/// `GameConfig::snapshot_skip` for the cadence) used to re-`insert()` and
/// reclone every component unconditionally, even for entities whose data
/// hadn't changed since the last pass -- pure waste for mostly-static scenes (GUI panels,
/// HUDs). Takes `value` by reference and only clones it on the write path,
/// not the comparison path, so an unchanged component (the common case)
/// costs one comparison and zero allocations -- cloning first and comparing
/// after would pay the clone (heap `String`/`Vec` allocations for types like
/// `GuiButton`/`Sprite`) on every publish regardless of whether the result
/// is ever used. Bevy change detection is not otherwise relied on
/// downstream (`render_system` has no `Changed<T>` filters), so this is
/// purely an allocation/write-avoidance optimization, not a correctness
/// dependency -- do not build new logic on top of the change ticks this now
/// avoids bumping.
fn insert_if_changed<C: Component + PartialEq + Clone>(entity: &mut EntityWorldMut, value: &C) {
    if entity.get::<C>() != Some(value) {
        entity.insert(value.clone());
    }
}

/// Insert a clone of `component` if `Some` (skipping the write -- and the
/// clone -- when unchanged, see [`insert_if_changed`]), remove it if `None`.
/// The one piece of per-optional-field logic reconciliation needs beyond
/// "insert the whole bundle": a sim-side optional component that goes from
/// present to absent between snapshots (e.g. a `Tint` removed mid-game) must
/// be *removed* from the mirror, not left stale -- a bundle `.insert()`
/// alone never removes anything. The removal path always runs
/// unconditionally (never diff-guarded) -- `EntityWorldMut::remove` is
/// already a no-op when the component is absent, so there's no
/// unnecessary-write cost to skip.
fn set_optional<C: Component + PartialEq + Clone>(entity: &mut EntityWorldMut, value: Option<&C>) {
    match value {
        Some(c) => insert_if_changed(entity, c),
        None => {
            entity.remove::<C>();
        }
    }
}

/// Shared upsert (spawn-or-update) + despawn-on-vanish reconciliation loop,
/// used identically by every category's `reconcile_*` function below: spawn
/// a mirror (tagged with `Marker::default()`) for every sim id not yet seen,
/// update (diffed field-by-field via `insert_if_changed`/`set_optional` --
/// each write skipped when the incoming value equals what's already on the
/// mirror) every component on ids already mirrored, and despawn any
/// previously-mirrored id absent from `entries` this pass. The only
/// per-category difference left as a closure is `apply`, since which
/// components to write genuinely varies (e.g. map sprites also carry
/// `Scale`/`Rotation`, screen categories don't) -- id lookup, spawn-on-new-id,
/// and despawn-on-vanish are byte-for-byte identical across categories and
/// live entirely in this function.
fn reconcile<Marker: Component + Default, E: SimEntry>(
    world: &mut World,
    id_map: &mut FxHashMap<u64, Entity>,
    seen: &mut FxHashSet<u64>,
    entries: &[E],
    apply: impl Fn(&E, &mut EntityWorldMut),
) {
    seen.clear();

    for entry in entries {
        let sim_entity = entry.sim_entity();
        let id = sim_entity.to_bits();
        seen.insert(id);

        let existing = id_map.get(&id).copied();
        let mut entity_mut = match existing.and_then(|e| world.get_entity_mut(e).ok()) {
            Some(entity_mut) => entity_mut,
            None => {
                let entity_mut = world.spawn((Marker::default(), SimMirror(sim_entity)));
                id_map.insert(id, entity_mut.id());
                entity_mut
            }
        };
        apply(entry, &mut entity_mut);
    }

    id_map.retain(|id, &mut mirror_entity| {
        if seen.contains(id) {
            true
        } else {
            world.despawn(mirror_entity);
            false
        }
    });
}

/// Reconcile the render world's map-sprite mirror entities against this
/// snapshot tick's `MapSpriteEntry` list.
///
/// A plain function (not a registered system) taking `&mut World` directly
/// -- called from `receive_snapshot`'s body, and callable directly by tests
/// with a bare `World`, bypassing `SnapshotConsumer`/the triple buffer
/// entirely.
pub fn reconcile_map_sprites(world: &mut World, entries: &[MapSpriteEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            map_sprites,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorMapSprite, _>(
            world,
            map_sprites,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.sprite);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
                set_optional::<Scale>(entity_mut, entry.scale.as_ref());
                set_optional::<Rotation>(entity_mut, entry.rotation.as_ref());
                set_optional::<EntityShader>(entity_mut, entry.shader.as_ref());
                set_optional::<Tint>(entity_mut, entry.tint.as_ref());
                set_optional::<Shadow>(entity_mut, entry.shadow.as_ref());
                set_optional::<GlobalTransform2D>(entity_mut, entry.global_transform.as_ref());
                let velocity = entry.velocity.map(MirrorVelocity);
                set_optional::<MirrorVelocity>(entity_mut, velocity.as_ref());
            },
        );
    });
}

/// Reconcile the render world's map-text mirror entities against this
/// snapshot tick's `MapTextEntry` list. Mirrors [`reconcile_map_sprites`]
/// exactly, minus `Scale`/`Rotation` (`DynamicText` has no scale/rotation
/// concept).
pub fn reconcile_map_texts(world: &mut World, entries: &[MapTextEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            map_texts,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorMapText, _>(
            world,
            map_texts,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.text);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
                set_optional::<EntityShader>(entity_mut, entry.shader.as_ref());
                set_optional::<Tint>(entity_mut, entry.tint.as_ref());
                set_optional::<Shadow>(entity_mut, entry.shadow.as_ref());
                set_optional::<GlobalTransform2D>(entity_mut, entry.global_transform.as_ref());
                let velocity = entry.velocity.map(MirrorVelocity);
                set_optional::<MirrorVelocity>(entity_mut, velocity.as_ref());
            },
        );
    });
}

/// Reconcile the render world's screen-sprite mirror entities against this
/// snapshot tick's `ScreenSpriteEntry` list. Simpler than the map-space
/// categories: screen space has no `Scale`/`Rotation`/`EntityShader`/
/// `GlobalTransform2D`/velocity concept (mirrors `ScreenSpriteEntry`'s own
/// reduced field set).
pub fn reconcile_screen_sprites(world: &mut World, entries: &[ScreenSpriteEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            screen_sprites,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorScreenSprite, _>(
            world,
            screen_sprites,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.sprite);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
                set_optional::<Tint>(entity_mut, entry.tint.as_ref());
                set_optional::<Shadow>(entity_mut, entry.shadow.as_ref());
            },
        );
    });
}

/// Reconcile the render world's screen-text mirror entities against this
/// snapshot tick's `ScreenTextEntry` list. Mirrors
/// [`reconcile_screen_sprites`] exactly, swapping `Sprite` for `DynamicText`.
pub fn reconcile_screen_texts(world: &mut World, entries: &[ScreenTextEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            screen_texts,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorScreenText, _>(
            world,
            screen_texts,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.text);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
                set_optional::<Tint>(entity_mut, entry.tint.as_ref());
                set_optional::<Shadow>(entity_mut, entry.shadow.as_ref());
            },
        );
    });
}

/// Reconcile the render world's GUI-window mirror entities against this
/// snapshot tick's `GuiWindowEntry` list. Simpler than every prior category:
/// `GuiWindowEntry` has no `Option<...>` fields, so the whole bundle is a
/// single `.insert()`, no `set_optional` calls needed.
pub fn reconcile_gui_windows(world: &mut World, entries: &[GuiWindowEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            gui_windows,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorGuiWindow, _>(
            world,
            gui_windows,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.window);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
            },
        );
    });
}

/// Reconcile the render world's GUI-button mirror entities against this
/// snapshot tick's `GuiButtonEntry` list. `GuiButton` and `GuiInteractable`
/// are two components required to coexist on the same sim entity (not a
/// cross-entity join); both land on the mirror via one whole-bundle
/// `.insert()`, same shape as any other multi-component apply closure here.
pub fn reconcile_gui_buttons(world: &mut World, entries: &[GuiButtonEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            gui_buttons,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorGuiButton, _>(
            world,
            gui_buttons,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.button);
                insert_if_changed(entity_mut, &entry.interactable);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
            },
        );
    });
}

/// Reconcile the render world's GUI-label mirror entities against this
/// snapshot tick's `GuiLabelEntry` list. Mirrors [`reconcile_gui_windows`]
/// exactly, swapping `GuiWindow` for `GuiLabel`.
pub fn reconcile_gui_labels(world: &mut World, entries: &[GuiLabelEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            gui_labels,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorGuiLabel, _>(
            world,
            gui_labels,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.label);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
            },
        );
    });
}

/// Reconcile the render world's GUI-progress-bar mirror entities against
/// this snapshot tick's `GuiProgressBarEntry` list. Mirrors
/// [`reconcile_gui_windows`] exactly, swapping `GuiWindow` for
/// `GuiProgressBar`.
pub fn reconcile_gui_progress_bars(world: &mut World, entries: &[GuiProgressBarEntry]) {
    world.resource_scope::<SimIdMap, _>(|world, mut id_map| {
        let SimIdMap {
            gui_progress_bars,
            scratch_seen,
            ..
        } = &mut *id_map;
        reconcile::<MirrorGuiProgressBar, _>(
            world,
            gui_progress_bars,
            scratch_seen,
            entries,
            |entry, entity_mut| {
                insert_if_changed(entity_mut, &entry.progress_bar);
                insert_if_changed(entity_mut, &entry.position);
                insert_if_changed(entity_mut, &entry.z_index);
            },
        );
    });
}

type MirrorMapSpriteQueryData = (
    &'static SimMirror,
    &'static Sprite,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static Scale>,
    Option<&'static Rotation>,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static Shadow>,
    Option<&'static GlobalTransform2D>,
    Option<&'static MirrorVelocity>,
);

type MirrorMapTextQueryData = (
    &'static SimMirror,
    &'static DynamicText,
    &'static MapPosition,
    &'static ZIndex,
    Option<&'static EntityShader>,
    Option<&'static Tint>,
    Option<&'static Shadow>,
    Option<&'static GlobalTransform2D>,
    Option<&'static MirrorVelocity>,
);

type MirrorScreenSpriteQueryData = (
    &'static SimMirror,
    &'static Sprite,
    &'static ScreenPosition,
    &'static ZIndex,
    Option<&'static Tint>,
    Option<&'static Shadow>,
);

type MirrorScreenTextQueryData = (
    &'static SimMirror,
    &'static DynamicText,
    &'static ScreenPosition,
    &'static ZIndex,
    Option<&'static Tint>,
    Option<&'static Shadow>,
);

type MirrorGuiWindowQueryData = (
    &'static SimMirror,
    &'static GuiWindow,
    &'static ScreenPosition,
    &'static ZIndex,
);

type MirrorGuiButtonQueryData = (
    &'static SimMirror,
    &'static GuiButton,
    &'static GuiInteractable,
    &'static ScreenPosition,
    &'static ZIndex,
);

type MirrorGuiLabelQueryData = (
    &'static SimMirror,
    &'static GuiLabel,
    &'static ScreenPosition,
    &'static ZIndex,
);

type MirrorGuiProgressBarQueryData = (
    &'static SimMirror,
    &'static GuiProgressBar,
    &'static ScreenPosition,
    &'static ZIndex,
);

/// Bundled mirror-entity draw-prep queries for `render_system`, mirroring
/// `DrawableSnapshotQueries`' bundling convention
/// (`src/resources/drawable_snapshot.rs`) and `RenderResources`/
/// `DebugResources`' (`src/systems/render/mod.rs`) -- named query-data type
/// aliases plus one `#[derive(SystemParam)]` struct, rather than 4 raw
/// `Query<...>` parameters directly on `render_system`'s already-long
/// signature.
#[derive(SystemParam)]
pub struct MirrorQueries<'w, 's> {
    pub map_sprites: Query<'w, 's, MirrorMapSpriteQueryData, With<MirrorMapSprite>>,
    pub map_texts: Query<'w, 's, MirrorMapTextQueryData, With<MirrorMapText>>,
    pub screen_sprites: Query<'w, 's, MirrorScreenSpriteQueryData, With<MirrorScreenSprite>>,
    pub screen_texts: Query<'w, 's, MirrorScreenTextQueryData, With<MirrorScreenText>>,
    pub gui_windows: Query<'w, 's, MirrorGuiWindowQueryData, With<MirrorGuiWindow>>,
    pub gui_buttons: Query<'w, 's, MirrorGuiButtonQueryData, With<MirrorGuiButton>>,
    pub gui_labels: Query<'w, 's, MirrorGuiLabelQueryData, With<MirrorGuiLabel>>,
    pub gui_progress_bars: Query<'w, 's, MirrorGuiProgressBarQueryData, With<MirrorGuiProgressBar>>,
}

#[cfg(test)]
mod mirror_tests {
    use super::*;
    use crate::components::guiinteractable::GuiWidgetState;
    use raylib::prelude::Vector2;
    use std::sync::Arc;

    fn new_test_world() -> World {
        let mut world = World::new();
        world.insert_resource(SimIdMap::default());
        world
    }

    /// Spawn + despawn + spawn on a scratch `World` to get two real
    /// `Entity` values with the same index but different generation --
    /// exactly the case reconciliation must treat as vanish-then-spawn, not
    /// an update of a stale mirror. Shared by every category's
    /// generation-reuse test.
    fn generation_reuse_pair() -> (Entity, Entity) {
        let mut scratch = World::new();
        let e1 = scratch.spawn_empty().id();
        scratch.despawn(e1);
        let e2 = scratch.spawn_empty().id();
        assert_ne!(
            e1.to_bits(),
            e2.to_bits(),
            "premise: generation must differ"
        );
        (e1, e2)
    }

    fn make_map_sprite_entry(entity: Entity, z_index: f32) -> MapSpriteEntry {
        MapSpriteEntry {
            entity,
            sprite: Sprite {
                tex_key: "test".into(),
                width: 16.0,
                height: 16.0,
                offset: Vector2::zero(),
                origin: Vector2::zero(),
                flip_h: false,
                flip_v: false,
            },
            position: MapPosition::from_vec(Vector2::new(1.0, 2.0)),
            z_index: ZIndex(z_index),
            scale: None,
            rotation: None,
            shader: None,
            tint: None,
            shadow: None,
            global_transform: None,
            velocity: None,
        }
    }

    #[test]
    fn spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();
        let entries = vec![make_map_sprite_entry(sim_entity, 1.0)];

        reconcile_map_sprites(&mut world, &entries);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.map_sprites.len(), 1);
        let mirror_entity = *id_map.map_sprites.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorMapSprite>(mirror_entity).is_some());
        assert_eq!(world.get::<ZIndex>(mirror_entity).unwrap().0, 1.0);
        assert_eq!(
            world.get::<Sprite>(mirror_entity).unwrap().tex_key.as_ref(),
            "test"
        );
    }

    #[test]
    fn updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 1.0)]);
        let mirror_entity_first = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&sim_entity.to_bits())
            .unwrap();

        let mut second = make_map_sprite_entry(sim_entity, 2.0);
        second.sprite.tex_key = "changed".into();
        reconcile_map_sprites(&mut world, &[second]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.map_sprites.len(), 1);
        let mirror_entity_second = *id_map.map_sprites.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(
            mirror_entity_first, mirror_entity_second,
            "reconciliation must update the same mirror entity, not spawn a new one"
        );
        assert_eq!(world.get::<ZIndex>(mirror_entity_second).unwrap().0, 2.0);
        assert_eq!(
            world
                .get::<Sprite>(mirror_entity_second)
                .unwrap()
                .tex_key
                .as_ref(),
            "changed"
        );
    }

    #[test]
    fn unchanged_republish_does_not_bump_component_change_tick() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&sim_entity.to_bits())
            .unwrap();
        // Advance the change-tick baseline (mirrors clear_trackers() being
        // called once per render frame in the real schedule) so the next
        // reconcile's is_changed() check reflects only writes made after
        // this point, not the initial spawn-time insert above.
        world.clear_trackers();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 1.0)]);

        assert!(
            !world
                .entity(mirror_entity)
                .get_ref::<Sprite>()
                .unwrap()
                .is_changed(),
            "reconciling with an identical entry must not rewrite (and so must not bump the \
             change tick of) an already-up-to-date component"
        );
        assert!(
            !world
                .entity(mirror_entity)
                .get_ref::<ZIndex>()
                .unwrap()
                .is_changed(),
            "same, for a second field on the same mirror entity"
        );
    }

    #[test]
    fn changed_republish_does_bump_component_change_tick() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&sim_entity.to_bits())
            .unwrap();
        world.clear_trackers();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 2.0)]);

        assert!(
            world
                .entity(mirror_entity)
                .get_ref::<ZIndex>()
                .unwrap()
                .is_changed(),
            "a genuinely changed field must still be rewritten"
        );
    }

    #[test]
    fn optional_component_removed_when_source_goes_none() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        let mut with_tint = make_map_sprite_entry(sim_entity, 1.0);
        with_tint.tint = Some(Tint::default());
        with_tint.shadow = Some(Shadow {
            offset: Vector2::new(1.0, 1.0),
            color: raylib::prelude::Color::BLACK,
        });
        reconcile_map_sprites(&mut world, &[with_tint]);

        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&sim_entity.to_bits())
            .unwrap();
        assert!(world.get::<Tint>(mirror_entity).is_some());
        assert!(world.get::<Shadow>(mirror_entity).is_some());

        let without_tint = make_map_sprite_entry(sim_entity, 1.0);
        reconcile_map_sprites(&mut world, &[without_tint]);

        assert!(
            world.get::<Tint>(mirror_entity).is_none(),
            "Tint must be removed from the mirror once the source entry no longer carries one"
        );
        assert!(
            world.get::<Shadow>(mirror_entity).is_none(),
            "Shadow must be removed from the mirror once the source entry no longer carries one"
        );
    }

    #[test]
    fn despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&sim_entity.to_bits())
            .unwrap();
        assert!(world.get_entity(mirror_entity).is_ok());

        reconcile_map_sprites(&mut world, &[]);

        assert!(
            world.get_entity(mirror_entity).is_err(),
            "mirror entity must be despawned once its sim id no longer appears in the snapshot"
        );
        assert!(world.resource::<SimIdMap>().map_sprites.is_empty());
    }

    #[test]
    fn respawn_after_despawn_gets_new_mirror_not_reused_stale_one() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .map_sprites
            .get(&e1.to_bits())
            .unwrap();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(e2, 1.0)]);

        assert!(
            world.get_entity(mirror_1).is_err(),
            "the old mirror (for the despawned sim entity) must be despawned"
        );
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.map_sprites.contains_key(&e1.to_bits()));
        let mirror_2 = *id_map.map_sprites.get(&e2.to_bits()).unwrap();
        assert_ne!(
            mirror_1, mirror_2,
            "a genuinely new mirror must be spawned for e2"
        );
        assert_eq!(id_map.map_sprites.len(), 1);
    }

    // ---- map texts: mechanical repeat of the map-sprite quintet ----

    fn make_map_text_entry(entity: Entity, z_index: f32) -> MapTextEntry {
        MapTextEntry {
            entity,
            text: DynamicText::new("hi", "font", 16.0, raylib::prelude::Color::WHITE),
            position: MapPosition::from_vec(Vector2::new(1.0, 2.0)),
            z_index: ZIndex(z_index),
            shader: None,
            tint: None,
            shadow: None,
            global_transform: None,
            velocity: None,
        }
    }

    #[test]
    fn map_text_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_texts(&mut world, &[make_map_text_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.map_texts.len(), 1);
        let mirror_entity = *id_map.map_texts.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorMapText>(mirror_entity).is_some());
        assert_eq!(world.get::<ZIndex>(mirror_entity).unwrap().0, 1.0);
    }

    #[test]
    fn map_text_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_texts(&mut world, &[make_map_text_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .map_texts
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_map_texts(&mut world, &[make_map_text_entry(sim_entity, 2.0)]);
        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.map_texts.len(), 1);
        let mirror_second = *id_map.map_texts.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(mirror_first, mirror_second);
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn map_text_optional_component_removed_when_source_goes_none() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        let mut with_tint = make_map_text_entry(sim_entity, 1.0);
        with_tint.tint = Some(Tint::default());
        reconcile_map_texts(&mut world, &[with_tint]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_texts
            .get(&sim_entity.to_bits())
            .unwrap();
        assert!(world.get::<Tint>(mirror_entity).is_some());

        reconcile_map_texts(&mut world, &[make_map_text_entry(sim_entity, 1.0)]);
        assert!(world.get::<Tint>(mirror_entity).is_none());
    }

    #[test]
    fn map_text_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_map_texts(&mut world, &[make_map_text_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .map_texts
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_map_texts(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().map_texts.is_empty());
    }

    #[test]
    fn map_text_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_map_texts(&mut world, &[make_map_text_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .map_texts
            .get(&e1.to_bits())
            .unwrap();

        reconcile_map_texts(&mut world, &[make_map_text_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.map_texts.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.map_texts.get(&e2.to_bits()).unwrap());
        assert_eq!(id_map.map_texts.len(), 1);
    }

    // ---- screen sprites: mechanical repeat, reduced optional field set ----

    fn make_screen_sprite_entry(entity: Entity, z_index: f32) -> ScreenSpriteEntry {
        ScreenSpriteEntry {
            entity,
            sprite: Sprite {
                tex_key: "test".into(),
                width: 16.0,
                height: 16.0,
                offset: Vector2::zero(),
                origin: Vector2::zero(),
                flip_h: false,
                flip_v: false,
            },
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
            tint: None,
            shadow: None,
        }
    }

    #[test]
    fn screen_sprite_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.screen_sprites.len(), 1);
        let mirror_entity = *id_map.screen_sprites.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorScreenSprite>(mirror_entity).is_some());
    }

    #[test]
    fn screen_sprite_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .screen_sprites
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(sim_entity, 2.0)]);
        let id_map = world.resource::<SimIdMap>();
        let mirror_second = *id_map.screen_sprites.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(mirror_first, mirror_second);
        assert_eq!(id_map.screen_sprites.len(), 1);
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn screen_sprite_optional_component_removed_when_source_goes_none() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        let mut with_shadow = make_screen_sprite_entry(sim_entity, 1.0);
        with_shadow.shadow = Some(Shadow {
            offset: Vector2::new(1.0, 1.0),
            color: raylib::prelude::Color::BLACK,
        });
        reconcile_screen_sprites(&mut world, &[with_shadow]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .screen_sprites
            .get(&sim_entity.to_bits())
            .unwrap();
        assert!(world.get::<Shadow>(mirror_entity).is_some());

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(sim_entity, 1.0)]);
        assert!(world.get::<Shadow>(mirror_entity).is_none());
    }

    #[test]
    fn screen_sprite_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .screen_sprites
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_screen_sprites(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().screen_sprites.is_empty());
    }

    #[test]
    fn screen_sprite_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .screen_sprites
            .get(&e1.to_bits())
            .unwrap();

        reconcile_screen_sprites(&mut world, &[make_screen_sprite_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.screen_sprites.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.screen_sprites.get(&e2.to_bits()).unwrap());
    }

    // ---- screen texts: mechanical repeat ----

    fn make_screen_text_entry(entity: Entity, z_index: f32) -> ScreenTextEntry {
        ScreenTextEntry {
            entity,
            text: DynamicText::new("hi", "font", 16.0, raylib::prelude::Color::WHITE),
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
            tint: None,
            shadow: None,
        }
    }

    #[test]
    fn screen_text_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.screen_texts.len(), 1);
        let mirror_entity = *id_map.screen_texts.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorScreenText>(mirror_entity).is_some());
    }

    #[test]
    fn screen_text_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .screen_texts
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(sim_entity, 2.0)]);
        let id_map = world.resource::<SimIdMap>();
        let mirror_second = *id_map.screen_texts.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(mirror_first, mirror_second);
        assert_eq!(id_map.screen_texts.len(), 1);
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn screen_text_optional_component_removed_when_source_goes_none() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        let mut with_tint = make_screen_text_entry(sim_entity, 1.0);
        with_tint.tint = Some(Tint::default());
        reconcile_screen_texts(&mut world, &[with_tint]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .screen_texts
            .get(&sim_entity.to_bits())
            .unwrap();
        assert!(world.get::<Tint>(mirror_entity).is_some());

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(sim_entity, 1.0)]);
        assert!(world.get::<Tint>(mirror_entity).is_none());
    }

    #[test]
    fn screen_text_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .screen_texts
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_screen_texts(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().screen_texts.is_empty());
    }

    #[test]
    fn screen_text_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .screen_texts
            .get(&e1.to_bits())
            .unwrap();

        reconcile_screen_texts(&mut world, &[make_screen_text_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.screen_texts.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.screen_texts.get(&e2.to_bits()).unwrap());
    }

    /// Reconciling two DIFFERENT categories with the same-shaped ids must
    /// never let one category's despawn-on-vanish pass touch another's
    /// mirrors. Spawns a map-sprite and a screen-text mirror in the same
    /// tick, then reconciles map sprites alone with an empty list and
    /// asserts the screen-text mirror survives untouched.
    #[test]
    fn despawn_on_vanish_is_scoped_to_its_own_category() {
        let mut world = new_test_world();
        let sprite_entity = world.spawn_empty().id();
        let text_entity = world.spawn_empty().id();

        reconcile_map_sprites(&mut world, &[make_map_sprite_entry(sprite_entity, 1.0)]);
        reconcile_screen_texts(&mut world, &[make_screen_text_entry(text_entity, 1.0)]);
        let text_mirror = *world
            .resource::<SimIdMap>()
            .screen_texts
            .get(&text_entity.to_bits())
            .unwrap();

        // Reconcile map sprites with an empty list -- must despawn the
        // map-sprite mirror, but must NOT touch the screen-text mirror.
        reconcile_map_sprites(&mut world, &[]);

        assert!(world.resource::<SimIdMap>().map_sprites.is_empty());
        assert!(world.get_entity(text_mirror).is_ok());
        assert_eq!(world.resource::<SimIdMap>().screen_texts.len(), 1);
    }

    // ---- GUI windows: mechanical repeat, no optional fields at all ----

    fn make_gui_window_entry(entity: Entity, z_index: f32) -> GuiWindowEntry {
        GuiWindowEntry {
            entity,
            window: GuiWindow::new(100.0, 50.0),
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
        }
    }

    #[test]
    fn gui_window_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_windows.len(), 1);
        let mirror_entity = *id_map.gui_windows.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorGuiWindow>(mirror_entity).is_some());
        assert_eq!(world.get::<ZIndex>(mirror_entity).unwrap().0, 1.0);
    }

    #[test]
    fn gui_window_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .gui_windows
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(sim_entity, 2.0)]);
        let id_map = world.resource::<SimIdMap>();
        let mirror_second = *id_map.gui_windows.get(&sim_entity.to_bits()).unwrap();

        assert_eq!(mirror_first, mirror_second);
        assert_eq!(id_map.gui_windows.len(), 1);
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn gui_window_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .gui_windows
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_gui_windows(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().gui_windows.is_empty());
    }

    #[test]
    fn gui_window_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .gui_windows
            .get(&e1.to_bits())
            .unwrap();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.gui_windows.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.gui_windows.get(&e2.to_bits()).unwrap());
    }

    // ---- GUI buttons: mechanical repeat, plus a two-component bundle test ----

    fn make_gui_button_entry(entity: Entity, z_index: f32) -> GuiButtonEntry {
        GuiButtonEntry {
            entity,
            button: GuiButton::new(100.0, 30.0, "Play"),
            interactable: GuiInteractable {
                size: Vector2::new(100.0, 30.0),
                state: GuiWidgetState::Normal,
                on_click_callback: None,
                on_rust_callback: None,
            },
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
        }
    }

    #[test]
    fn gui_button_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_buttons.len(), 1);
        let mirror_entity = *id_map.gui_buttons.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorGuiButton>(mirror_entity).is_some());
        assert_eq!(
            world.get::<GuiButton>(mirror_entity).unwrap().caption,
            "Play"
        );
        assert_eq!(
            world.get::<GuiInteractable>(mirror_entity).unwrap().state,
            GuiWidgetState::Normal
        );
    }

    /// `GuiButton`+`GuiInteractable` are two required components on one
    /// mirror entity (not a cross-entity join) -- reconciling with both
    /// `button.theme_key` and `interactable.state` changed must update BOTH
    /// on the same mirror entity in one pass, proving the whole-bundle
    /// `.insert()` doesn't drop or stagger either field.
    #[test]
    fn gui_button_updates_both_bundled_components_together() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .gui_buttons
            .get(&sim_entity.to_bits())
            .unwrap();

        let mut second = make_gui_button_entry(sim_entity, 2.0);
        second.button.theme_key = Arc::from("changed");
        second.interactable.state = GuiWidgetState::Hovered;
        reconcile_gui_buttons(&mut world, &[second]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_buttons.len(), 1);
        let mirror_second = *id_map.gui_buttons.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(
            mirror_first, mirror_second,
            "must update the same mirror, not spawn a new one"
        );

        assert_eq!(
            &*world.get::<GuiButton>(mirror_second).unwrap().theme_key,
            "changed"
        );
        assert_eq!(
            world.get::<GuiInteractable>(mirror_second).unwrap().state,
            GuiWidgetState::Hovered
        );
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn gui_button_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .gui_buttons
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_gui_buttons(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().gui_buttons.is_empty());
    }

    #[test]
    fn gui_button_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .gui_buttons
            .get(&e1.to_bits())
            .unwrap();

        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.gui_buttons.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.gui_buttons.get(&e2.to_bits()).unwrap());
    }

    // ---- GUI labels: mechanical repeat ----

    fn make_gui_label_entry(entity: Entity, z_index: f32) -> GuiLabelEntry {
        GuiLabelEntry {
            entity,
            label: GuiLabel::new(80.0, 20.0, "Score"),
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
        }
    }

    #[test]
    fn gui_label_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_labels(&mut world, &[make_gui_label_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_labels.len(), 1);
        let mirror_entity = *id_map.gui_labels.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorGuiLabel>(mirror_entity).is_some());
        assert_eq!(
            world.get::<GuiLabel>(mirror_entity).unwrap().caption,
            "Score"
        );
    }

    #[test]
    fn gui_label_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_labels(&mut world, &[make_gui_label_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .gui_labels
            .get(&sim_entity.to_bits())
            .unwrap();

        let mut second = make_gui_label_entry(sim_entity, 2.0);
        second.label.caption = "Changed".to_string();
        reconcile_gui_labels(&mut world, &[second]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_labels.len(), 1);
        let mirror_second = *id_map.gui_labels.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(mirror_first, mirror_second);
        assert_eq!(
            world.get::<GuiLabel>(mirror_second).unwrap().caption,
            "Changed"
        );
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn gui_label_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_labels(&mut world, &[make_gui_label_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .gui_labels
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_gui_labels(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().gui_labels.is_empty());
    }

    #[test]
    fn gui_label_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_gui_labels(&mut world, &[make_gui_label_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .gui_labels
            .get(&e1.to_bits())
            .unwrap();

        reconcile_gui_labels(&mut world, &[make_gui_label_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.gui_labels.contains_key(&e1.to_bits()));
        assert_ne!(mirror_1, *id_map.gui_labels.get(&e2.to_bits()).unwrap());
    }

    // ---- GUI progress bars: mechanical repeat ----

    fn make_gui_progress_bar_entry(entity: Entity, z_index: f32) -> GuiProgressBarEntry {
        GuiProgressBarEntry {
            entity,
            progress_bar: GuiProgressBar::new(50.0, 8.0, 3.0, 10.0),
            position: ScreenPosition::new(1.0, 2.0),
            z_index: ZIndex(z_index),
        }
    }

    #[test]
    fn gui_progress_bar_spawns_mirror_on_new_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_progress_bars(&mut world, &[make_gui_progress_bar_entry(sim_entity, 1.0)]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_progress_bars.len(), 1);
        let mirror_entity = *id_map.gui_progress_bars.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(world.get::<SimMirror>(mirror_entity).unwrap().0, sim_entity);
        assert!(world.get::<MirrorGuiProgressBar>(mirror_entity).is_some());
        assert_eq!(
            world.get::<GuiProgressBar>(mirror_entity).unwrap().value,
            3.0
        );
    }

    #[test]
    fn gui_progress_bar_updates_existing_mirror_in_place() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_progress_bars(&mut world, &[make_gui_progress_bar_entry(sim_entity, 1.0)]);
        let mirror_first = *world
            .resource::<SimIdMap>()
            .gui_progress_bars
            .get(&sim_entity.to_bits())
            .unwrap();

        let mut second = make_gui_progress_bar_entry(sim_entity, 2.0);
        second.progress_bar.value = 7.0;
        reconcile_gui_progress_bars(&mut world, &[second]);

        let id_map = world.resource::<SimIdMap>();
        assert_eq!(id_map.gui_progress_bars.len(), 1);
        let mirror_second = *id_map.gui_progress_bars.get(&sim_entity.to_bits()).unwrap();
        assert_eq!(mirror_first, mirror_second);
        assert_eq!(
            world.get::<GuiProgressBar>(mirror_second).unwrap().value,
            7.0
        );
        assert_eq!(world.get::<ZIndex>(mirror_second).unwrap().0, 2.0);
    }

    #[test]
    fn gui_progress_bar_despawns_mirror_on_vanished_id() {
        let mut world = new_test_world();
        let sim_entity = world.spawn_empty().id();

        reconcile_gui_progress_bars(&mut world, &[make_gui_progress_bar_entry(sim_entity, 1.0)]);
        let mirror_entity = *world
            .resource::<SimIdMap>()
            .gui_progress_bars
            .get(&sim_entity.to_bits())
            .unwrap();

        reconcile_gui_progress_bars(&mut world, &[]);

        assert!(world.get_entity(mirror_entity).is_err());
        assert!(world.resource::<SimIdMap>().gui_progress_bars.is_empty());
    }

    #[test]
    fn gui_progress_bar_respawn_after_despawn_gets_new_mirror() {
        let mut world = new_test_world();
        let (e1, e2) = generation_reuse_pair();

        reconcile_gui_progress_bars(&mut world, &[make_gui_progress_bar_entry(e1, 1.0)]);
        let mirror_1 = *world
            .resource::<SimIdMap>()
            .gui_progress_bars
            .get(&e1.to_bits())
            .unwrap();

        reconcile_gui_progress_bars(&mut world, &[make_gui_progress_bar_entry(e2, 1.0)]);

        assert!(world.get_entity(mirror_1).is_err());
        let id_map = world.resource::<SimIdMap>();
        assert!(!id_map.gui_progress_bars.contains_key(&e1.to_bits()));
        assert_ne!(
            mirror_1,
            *id_map.gui_progress_bars.get(&e2.to_bits()).unwrap()
        );
    }

    /// Sibling to `despawn_on_vanish_is_scoped_to_its_own_category` covering
    /// the 4 new GUI categories specifically: reconciling `gui_windows`
    /// empty must not touch a co-existing `gui_buttons` mirror.
    #[test]
    fn gui_despawn_on_vanish_is_scoped_to_its_own_category() {
        let mut world = new_test_world();
        let window_entity = world.spawn_empty().id();
        let button_entity = world.spawn_empty().id();

        reconcile_gui_windows(&mut world, &[make_gui_window_entry(window_entity, 1.0)]);
        reconcile_gui_buttons(&mut world, &[make_gui_button_entry(button_entity, 1.0)]);
        let button_mirror = *world
            .resource::<SimIdMap>()
            .gui_buttons
            .get(&button_entity.to_bits())
            .unwrap();

        reconcile_gui_windows(&mut world, &[]);

        assert!(world.resource::<SimIdMap>().gui_windows.is_empty());
        assert!(world.get_entity(button_mirror).is_ok());
        assert_eq!(world.resource::<SimIdMap>().gui_buttons.len(), 1);
    }
}
