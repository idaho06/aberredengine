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

use bevy_ecs::prelude::*;
use raylib::prelude::Vector2;

use crate::components::boxcollider::BoxCollider;
use crate::components::gridlayout::{GridLayout, GridLayoutData, GridValue};
use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::signals::Signals;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;

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
                eprintln!(
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
                    tex_key: cell.texture_key.clone(),
                    width: layout_data.cell_width,
                    height: layout_data.cell_height,
                    offset: Vector2::zero(),
                    origin: Vector2 {
                        x: layout_data.cell_width * 0.5,
                        y: layout_data.cell_height * 0.5,
                    },
                    flip_h: false,
                    flip_v: false,
                },
                BoxCollider {
                    size: Vector2 {
                        x: layout_data.cell_width,
                        y: layout_data.cell_height,
                    },
                    offset: Vector2::zero(),
                    origin: Vector2 {
                        x: layout_data.cell_width * 0.5,
                        y: layout_data.cell_height * 0.5,
                    },
                },
                signals,
            ));
        }
        grid_layout.spawned = true;

        eprintln!(
            "Spawned grid layout from {} with group '{}'",
            grid_layout.path, grid_layout.group
        );
    }
}
