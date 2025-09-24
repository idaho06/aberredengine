use crate::resources::debugmode::DebugMode;
use bevy_ecs::observer::Trigger;
use bevy_ecs::prelude::*;

#[derive(Event, Debug, Clone, Copy)]
pub struct SwitchDebugEvent {}

pub fn observe_switch_debug_event(
    _trigger: Trigger<SwitchDebugEvent>,
    mut commands: Commands,
    debug_mode: Option<Res<DebugMode>>,
) {
    // This observer is triggered when a SwitchDebugEvent is fired.
    // It toggles the DebugMode resource.
    println!("SwitchDebugEvent triggered");

    // Toggle the DebugMode resource
    if debug_mode.is_some() {
        // If it exists, we remove it
        commands.remove_resource::<DebugMode>();
        eprintln!("Debug mode disabled");
    } else {
        eprintln!("Debug mode resource not found, creating new one");
        commands.insert_resource(DebugMode {});
    }
}
