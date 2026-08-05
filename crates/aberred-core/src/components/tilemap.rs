use bevy_ecs::prelude::Component;

/// Marks an entity as a tilemap root. A system watches for `Added<TileMap>`,
/// loads the PNG + JSON from `path`, spawns tile entities as `ChildOf` children,
/// and inserts a default `MapPosition` on the root if none is present.
///
/// The root entity can carry `MapPosition`, `Scale`, and `Rotation` to
/// transform the whole tilemap as a unit.
#[derive(Component, Clone, Debug)]
pub struct TileMap {
    pub path: String,
}

impl TileMap {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}
