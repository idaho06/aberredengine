//! Event and observer to toggle debug visualization and diagnostics.
//!
//! Emitting a [`DebugToggled`] flips the presence of the [`DebugMode`]
//! resource. Systems that render overlays or print extra diagnostics can gate
//! their behavior on this resource.
use crate::resources::debugmode::DebugMode;
use bevy_ecs::observer::On;
use bevy_ecs::prelude::*;
use log::{debug, info};

/// Event used to toggle the [`DebugMode`] resource on/off.
///
/// This carries no data; the observer simply switches the presence of the
/// resource.
#[derive(Event, Debug, Clone, Copy)]
pub struct DebugToggled {}

/// Observer that toggles the [`DebugMode`] resource.
///
/// - If `DebugMode` is present, it is removed (debug disabled).
/// - If absent, it is inserted (debug enabled).
pub fn debug_toggled_observer(
    _trigger: On<DebugToggled>,
    mut commands: Commands,
    debug_mode: Option<Res<DebugMode>>,
) {
    debug!("DebugToggled triggered");

    // Toggle the DebugMode resource
    if debug_mode.is_some() {
        // If it exists, we remove it
        commands.remove_resource::<DebugMode>();
        info!("Debug mode disabled");
    } else {
        info!("Debug mode enabled");
        commands.insert_resource(DebugMode {});
    }
}
