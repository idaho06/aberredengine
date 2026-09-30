//! Grid layout spawning system.
//!
//! The [`gridlayout_spawn_system`] processes newly added [`GridLayout`]
//! components, loads their JSON data, and spawns child entities for each
//! cell. Spawned entities receive [`MapPosition`], [`Sprite`], [`BoxCollider`],
//! [`Signals`], [`Group`], and [`ZIndex`] components based on the layout data.
//!
//! # JSON Format
//!
//! The JSON file defines a grid with a legend mapping characters to cell types:
//!
//! ```json
//! {
//!   "offset_x": 48.0,
//!   "offset_y": 80.0,
//!   "cell_width": 56.0,
//!   "cell_height": 24.0,
//!   "grid": ["RRGGBB", "YYPPMM"],
//!   "legend": {
//!     "R": { "texture_key": "brick_red", "properties": { "hp": 1, "points": 10 } }
//!   }
//! }
//! ```
//!
//! # Related
//!
//! - [`crate::components::gridlayout::GridLayout`] – the trigger component
//! - [`crate::components::gridlayout::GridLayoutData`] – the parsed JSON structure

use std::sync::Arc;

use bevy_ecs::prelude::*;
use crate::math::Vec2;

use crate::components::boxcollider::BoxCollider;
use crate::components::gridlayout::{GridLayout, GridLayoutData, GridValue};
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use log::{error, info};

/// System that processes GridLayout components and spawns child entities accordingly.
pub fn gridlayout_spawn_system(
    mut commands: Commands,
    mut query: Query<&mut GridLayout, Added<GridLayout>>,
) {
    for mut grid_layout in query.iter_mut() {
        if grid_layout.spawned {
            continue; // Skip if already spawned
        }

        // Load the grid layout data from the specified JSON file
        let layout_data = match GridLayoutData::load_from_file(&grid_layout.path) {
            Ok(data) => data,
            Err(err) => {
                error!(
                    "Failed to load grid layout from {}: {}",
                    grid_layout.path, err
                );
                grid_layout.spawned = true; // Prevent retrying
                continue;
            }
        };

        // Spawn entities for each cell in the grid
        for (x, y, cell) in layout_data.iter_cells() {
            let mut signals = Signals::default();

            // Copy all properties from the cell to signals
            for (key, value) in &cell.properties {
                match value {
                    GridValue::Int(v) => {
                        signals.set_integer(key, *v as i32);
                    }
                    GridValue::Float(v) => {
                        signals.set_scalar(key, *v as f32);
                    }
                    GridValue::String(v) => {
                        signals.set_string(key, v.clone());
                    }
                    GridValue::Bool(v) => {
                        if *v {
                            signals.set_flag(key);
                        }
                    }
                }
            }

            commands.spawn((
                Group::new(&grid_layout.group),
                MapPosition::new(x, y),
                ZIndex(grid_layout.z_index),
                Sprite {
                    tex_key: Arc::from(cell.texture_key.clone()),
                    width: layout_data.cell_width,
                    height: layout_data.cell_height,
                    offset: Vec2::ZERO,
                    origin: Vec2 {
                        x: layout_data.cell_width * 0.5,
                        y: layout_data.cell_height * 0.5,
                    },
                    flip_h: false,
                    flip_v: false,
                },
                BoxCollider {
                    size: Vec2 {
                        x: layout_data.cell_width,
                        y: layout_data.cell_height,
                    },
                    offset: Vec2::ZERO,
                    origin: Vec2 {
                        x: layout_data.cell_width * 0.5,
                        y: layout_data.cell_height * 0.5,
                    },
                },
                signals,
            ));
        }
        grid_layout.spawned = true;

        info!(
            "Spawned grid layout from {} with group '{}'",
            grid_layout.path, grid_layout.group
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: &str = r#"{
        "offset_x": 10.0,
        "offset_y": 20.0,
        "cell_width": 40.0,
        "cell_height": 16.0,
        "grid": ["R.G", " RX"],
        "legend": {
            "R": { "texture_key": "red", "properties": { "hp": 2, "speed": 1.5, "kind": "brick", "solid": true, "hidden": false } },
            "G": { "texture_key": "green", "properties": {} },
            "X": null
        }
    }"#;

    fn write_layout(dir: &tempfile::TempDir, json: &str) -> String {
        let path = dir.path().join("layout.json");
        std::fs::write(&path, json).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(gridlayout_spawn_system);
        schedule
    }

    fn cells(world: &mut World) -> Vec<(Vec2, String)> {
        let mut out: Vec<(Vec2, String)> = world
            .query::<(&MapPosition, &Sprite)>()
            .iter(world)
            .map(|(p, s)| (p.pos, s.tex_key.to_string()))
            .collect();
        out.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
        out
    }

    #[test]
    fn spawns_one_entity_per_mapped_cell_at_cell_centers() {
        let dir = tempfile::tempdir().unwrap();
        let mut world = World::new();
        let root = world
            .spawn(GridLayout::new(write_layout(&dir, LAYOUT), "bricks", 3.0))
            .id();
        schedule().run(&mut world);

        // '.', ' ' are not in the legend and 'X' maps to null: all skipped.
        assert_eq!(
            cells(&mut world),
            vec![
                (Vec2::new(30.0, 28.0), "red".to_string()),   // row 0, col 0
                (Vec2::new(70.0, 44.0), "red".to_string()),   // row 1, col 1
                (Vec2::new(110.0, 28.0), "green".to_string()), // row 0, col 2
            ]
        );
        assert!(world.get::<GridLayout>(root).unwrap().spawned);

        let mut q = world.query::<(&Group, &ZIndex, &Sprite, &BoxCollider)>();
        for (group, z, sprite, collider) in q.iter(&world) {
            assert_eq!((group.0.as_str(), z.0), ("bricks", 3.0));
            assert_eq!((sprite.width, sprite.height), (40.0, 16.0));
            assert_eq!(sprite.origin, Vec2::new(20.0, 8.0), "sprite centered on the cell");
            assert_eq!(collider.size, Vec2::new(40.0, 16.0));
            assert_eq!(collider.origin, Vec2::new(20.0, 8.0));
        }
    }

    #[test]
    fn cell_properties_become_typed_signals() {
        let dir = tempfile::tempdir().unwrap();
        let mut world = World::new();
        world.spawn(GridLayout::new(write_layout(&dir, LAYOUT), "bricks", 0.0));
        schedule().run(&mut world);

        let mut q = world.query::<(&Sprite, &Signals)>();
        let (_, red) = q.iter(&world).find(|(s, _)| &*s.tex_key == "red").unwrap();
        assert_eq!(red.get_integer("hp"), Some(2));
        assert_eq!(red.get_scalar("speed"), Some(1.5));
        assert_eq!(red.get_string("kind").map(String::as_str), Some("brick"));
        assert!(red.has_flag("solid"));
        assert!(!red.has_flag("hidden"), "a false bool sets no flag");
        let (_, green) = q.iter(&world).find(|(s, _)| &*s.tex_key == "green").unwrap();
        assert!(green.get_integer("hp").is_none());
    }

    #[test]
    fn layout_spawns_once_even_when_the_system_runs_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut world = World::new();
        world.spawn(GridLayout::new(write_layout(&dir, LAYOUT), "bricks", 0.0));
        let mut schedule = schedule();
        schedule.run(&mut world);
        schedule.run(&mut world);
        assert_eq!(cells(&mut world).len(), 3);
    }

    #[test]
    fn missing_or_invalid_file_spawns_nothing_and_is_not_retried() {
        let dir = tempfile::tempdir().unwrap();
        let mut world = World::new();
        let missing = world
            .spawn(GridLayout::new(
                dir.path().join("nope.json").to_string_lossy().into_owned(),
                "g",
                0.0,
            ))
            .id();
        let invalid = world
            .spawn(GridLayout::new(write_layout(&dir, "{ not json"), "g", 0.0))
            .id();
        schedule().run(&mut world);

        assert!(cells(&mut world).is_empty());
        assert!(world.get::<GridLayout>(missing).unwrap().spawned, "marked to prevent retry");
        assert!(world.get::<GridLayout>(invalid).unwrap().spawned);
    }
}
