//! Sim-entity-id -> mirror-entity lookup backing mirror-entity reconciliation
//! (`crate::systems::render::mirror`).

use bevy_ecs::prelude::{Entity, Resource};
use rustc_hash::{FxHashMap, FxHashSet};

/// Per-category sim-id -> render-mirror-entity maps. One `FxHashMap` field
/// per category (not one shared map) so a category's despawn-on-vanish pass
/// can only ever prune its own ids -- never let one category's reconcile
/// function reach another's field. `scratch_seen` is `reconcile`'s reusable
/// working set (cleared at the start of every call) -- kept here rather than
/// allocated fresh per call since the 8 `reconcile_*` entry points all run
/// sequentially from `receive_snapshot`, never concurrently, so one shared
/// scratch buffer is always free to reuse.
#[derive(Resource, Default)]
pub struct SimIdMap {
    pub map_sprites: FxHashMap<u64, Entity>,
    pub map_texts: FxHashMap<u64, Entity>,
    pub screen_sprites: FxHashMap<u64, Entity>,
    pub screen_texts: FxHashMap<u64, Entity>,
    pub gui_windows: FxHashMap<u64, Entity>,
    pub gui_buttons: FxHashMap<u64, Entity>,
    pub gui_labels: FxHashMap<u64, Entity>,
    pub gui_progress_bars: FxHashMap<u64, Entity>,
    pub(crate) scratch_seen: FxHashSet<u64>,
}
