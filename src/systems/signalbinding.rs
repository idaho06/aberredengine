//! Signal binding system for reactive UI updates.
//!
//! This module provides the system that synchronizes [`DynamicText`](crate::components::dynamictext::DynamicText)
//! components with signal values based on their [`SignalBinding`](crate::components::signalbinding::SignalBinding).

// Helper enum for returning either owned String or borrowed &str
enum Either<L, R> {
    Left(L),
    Right(R),
}

use std::sync::Arc;

use crate::components::dynamictext::DynamicText;
use crate::components::signalbinding::{SignalBinding, SignalSource};
use crate::components::signals::Signals;
use crate::resources::worldsignals::WorldSignals;
use bevy_ecs::prelude::*;

/// Updates [`DynamicText`](crate::components::dynamictext::DynamicText) content based on signal bindings.
///
/// This system queries all entities with both `DynamicText` and `SignalBinding` components,
/// reads the corresponding signal value (from either `WorldSignals` or an entity's `Signals`),
/// and updates the text content accordingly.
///
/// Supported signal types:
/// - **Integer** - Displayed as-is (e.g., `"42"`)
/// - **Scalar** - Displayed as a floating-point number (e.g., `"3.14"`)
/// - **String** - Displayed as-is
/// - **Flag** - Displayed as `"true"` if set
///
/// If a format string is specified in the binding, the value replaces the `{}` placeholder.
pub fn update_world_signals_binding_system(
    mut query: Query<(&mut DynamicText, &SignalBinding)>,
    world_signals: Res<WorldSignals>,
    signals_query: Query<&Signals>,
) {
    for (mut dynamic_text, signal_binding) in query.iter_mut() {
        let value_opt = match &signal_binding.source {
            SignalSource::World => {
                get_world_signal_as_str(&world_signals, &signal_binding.signal_key)
            }
            SignalSource::Entity(entity) => signals_query
                .get(*entity)
                .ok()
                .and_then(|signals| get_entity_signal_as_str(signals, &signal_binding.signal_key)),
        };

        if let Some(value) = value_opt {
            let new_content = match &signal_binding.format {
                Some(format_str) => match value {
                    Either::Left(ref s) => format_str.replace("{}", s),
                    Either::Right(s) => format_str.replace("{}", s),
                },
                None => match value {
                    Either::Left(ref s) => s.clone(),
                    Either::Right(s) => s.to_string(),
                },
            };
            if &*dynamic_text.content != new_content {
                dynamic_text.content = Arc::from(new_content);
            }
        }
    }
}

/// Converts a signal value from [`WorldSignals`] to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag.
/// Returns `None` if the signal key is not found.
fn get_world_signal_as_str<'a>(
    world_signals: &'a WorldSignals,
    signal_key: &str,
) -> Option<Either<String, &'a str>> {
    if let Some(signal_value) = world_signals.get_integer(signal_key) {
        return Some(Either::Left(signal_value.to_string()));
    }
    if let Some(signal_value) = world_signals.get_scalar(signal_key) {
        return Some(Either::Left(signal_value.to_string()));
    }
    if let Some(signal_value) = world_signals.get_string(signal_key) {
        return Some(Either::Right(signal_value.as_str()));
    }
    if world_signals.has_flag(signal_key) {
        return Some(Either::Right("true"));
    }
    None
}

/// Converts a signal value from an entity's [`Signals`] component to a string representation.
///
/// Tries each signal type in order: integer, scalar, string, flag.
/// Returns `None` if the signal key is not found.
fn get_entity_signal_as_str<'a>(
    signals: &'a Signals,
    signal_key: &str,
) -> Option<Either<String, &'a str>> {
    if let Some(signal_value) = signals.get_integer(signal_key) {
        return Some(Either::Left(signal_value.to_string()));
    }
    if let Some(signal_value) = signals.get_scalar(signal_key) {
        return Some(Either::Left(signal_value.to_string()));
    }
    if let Some(signal_value) = signals.get_string(signal_key) {
        return Some(Either::Right(signal_value.as_str()));
    }
    if signals.has_flag(signal_key) {
        return Some(Either::Right("true"));
    }
    None
}
