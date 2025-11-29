use crate::components::dynamictext::DynamicText;
use crate::components::signalbinding::{SignalBinding, SignalSource};
use crate::components::signals::Signals;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::*;

pub fn update_world_signals_binding_system(
    mut query: Query<(&mut DynamicText, &SignalBinding)>,
    world_signals: Res<WorldSignals>,
) {
    for (mut dynamic_text, signal_binding) in query.iter_mut() {
        let value_str =
            get_world_signal_as_string(&world_signals, signal_binding.signal_key.as_str());

        if let Some(value_str) = value_str {
            dynamic_text.content = match &signal_binding.format {
                Some(format_str) => format_str.replace("{}", &value_str),
                None => value_str,
            };
        }
    }
}

fn get_world_signal_as_string(world_signals: &WorldSignals, signal_key: &str) -> Option<String> {
    if let Some(signal_value) = world_signals.get_integer(signal_key) {
        return Some(signal_value.to_string());
    }
    if let Some(signal_value) = world_signals.get_scalar(signal_key) {
        return Some(signal_value.to_string());
    }
    if let Some(signal_value) = world_signals.get_string(signal_key) {
        return Some(signal_value.clone());
    }
    if world_signals.has_flag(signal_key) {
        return Some("true".to_string());
    }
    None
}
