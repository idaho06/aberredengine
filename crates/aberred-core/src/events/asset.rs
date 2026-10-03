//! Asset load completion events.
//!
//! [`AssetLoaded`] and [`AssetLoadFailed`] are triggered once per load
//! command when its reply arrives from the render or audio thread, after the
//! reply's data (texture dims, font metrics) is stored. Observe them with
//! `aberredengine::EngineBuilder::add_observer`.

use bevy_ecs::prelude::*;

use crate::protocol::asset_kind::AssetKind;

/// Triggered once when a queued asset load succeeds.
#[derive(Event, Debug, Clone)]
pub struct AssetLoaded {
    /// The kind of asset that loaded.
    pub kind: AssetKind,
    /// The key the asset is stored under.
    pub key: String,
}

/// Triggered once when a queued asset load fails (missing or invalid file).
/// The engine has already logged the error.
#[derive(Event, Debug, Clone)]
pub struct AssetLoadFailed {
    /// The kind of asset that failed to load.
    pub kind: AssetKind,
    /// The key the asset was to be stored under.
    pub key: String,
    /// What went wrong.
    pub error: String,
}
