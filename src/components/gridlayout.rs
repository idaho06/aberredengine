//! Grid layout component for data-driven entity spawning.
//!
//! The [`GridLayout`] component references a JSON file describing a grid of
//! cells. When the component is added, the
//! [`gridlayout_spawn_system`](crate::systems::gridlayout::gridlayout_spawn_system)
//! reads the file and spawns entities for each non-empty cell with the
//! specified texture, group, and custom properties.
//!
//! This is useful for tile-based games where level layouts are defined
//! externally (e.g., Arkanoid brick patterns, puzzle grids).

use bevy_ecs::prelude::*;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// A grid layout component that spawns child entities in a grid formation when spawned.
#[derive(Component, Debug, Clone)]
pub struct GridLayout {
    /// Path to the JSON file defining the grid layout.
    pub path: String,
    /// Group
    pub group: String,
    /// Z-Index
    pub z_index: f32,
    /// whether this layout has been initialized
    pub spawned: bool,
}

impl GridLayout {
    /// Creates a new GridLayout component.
    pub fn new(path: impl Into<String>, group: impl Into<String>, z_index: f32) -> Self {
        Self {
            path: path.into(),
            group: group.into(),
            z_index,
            spawned: false,
        }
    }
}

/// Structure representing the grid layout data loaded from JSON.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GridLayoutData {
    pub offset_x: f32,
    pub offset_y: f32,
    pub cell_width: f32,
    pub cell_height: f32,
    pub grid: Vec<String>,
    pub legend: FxHashMap<char, Option<GridCell>>,
}

/// Structure representing a single cell in the grid layout.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GridCell {
    pub texture_key: String,
    #[serde(default)]
    pub properties: FxHashMap<String, GridValue>,
}

/// Enum representing possible value types for grid cell properties.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum GridValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
}

/* impl GridValue {
    /// Attempts to retrieve the value as an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            GridValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Attempts to retrieve the value as a float.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            GridValue::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Attempts to retrieve the value as a string.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            GridValue::String(v) => Some(v),
            _ => None,
        }
    }

    /// Attempts to retrieve the value as a boolean.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            GridValue::Bool(v) => Some(*v),
            _ => None,
        }
    }
}
 */
impl GridLayoutData {
    /// Loads grid layout data from a JSON file at the specified path.
    pub fn load_from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file_content = std::fs::read_to_string(path)?;
        let layout_data: GridLayoutData = serde_json::from_str(&file_content)?;
        Ok(layout_data)
    }

    /// Iterate over all defined cells with their world positions
    pub fn iter_cells(&self) -> impl Iterator<Item = (f32, f32, &GridCell)> {
        self.grid.iter().enumerate().flat_map(move |(row, line)| {
            line.chars().enumerate().filter_map(move |(col, ch)| {
                if let Some(Some(cell)) = self.legend.get(&ch) {
                    let x =
                        self.offset_x + (col as f32 * self.cell_width) + (self.cell_width * 0.5);
                    let y =
                        self.offset_y + (row as f32 * self.cell_height) + (self.cell_height * 0.5);
                    Some((x, y, cell))
                } else {
                    None
                }
            })
        })
    }
}
