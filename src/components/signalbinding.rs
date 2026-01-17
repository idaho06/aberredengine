//! Signal binding component for reactive UI updates.
//!
//! This module provides a declarative way to bind [`DynamicText`](super::dynamictext::DynamicText)
//! components to signal values, enabling automatic text updates when signals change.
//!
//! # Signal Sources
//!
//! Bindings can read from two sources:
//! - [`WorldSignals`](crate::resources::worldsignals::WorldSignals) (default) – global signals like score, lives
//! - [`Signals`](super::signals::Signals) – per-entity signals from a specific entity
//!
//! # Supported Signal Types
//!
//! The binding system checks signal types in order: integer, scalar, string, flag.
//! Flags display as `"true"` when present.
//!
//! # Example
//!
//! ```ignore
//! // Display score from WorldSignals
//! commands.spawn((
//!     DynamicText::new("0", "arcade", 16.0, Color::WHITE),
//!     SignalBinding::new("score"),
//! ));
//!
//! // Display with custom format
//! commands.spawn((
//!     DynamicText::new("", "arcade", 16.0, Color::WHITE),
//!     SignalBinding::new("health").with_format("HP: {}"),
//! ));
//!
//! // Display from a specific entity's Signals
//! commands.spawn((
//!     DynamicText::new("", "arcade", 16.0, Color::WHITE),
//!     SignalBinding::new("hp").with_source_entity(player_entity),
//! ));
//! ```
//!
//! # Related
//!
//! - [`crate::systems::signalbinding::update_world_signals_binding_system`] – the update system
//! - [`crate::resources::worldsignals::WorldSignals`] – global signal storage
//! - [`super::signals::Signals`] – per-entity signal storage

use bevy_ecs::prelude::{Component, Entity};

/// Specifies where to read the signal value from.
#[derive(Clone, Debug)]
pub enum SignalSource {
    /// Read from the global [`WorldSignals`](crate::resources::worldsignals::WorldSignals) resource.
    World,
    /// Read from a specific entity's [`Signals`](super::signals::Signals) component.
    #[allow(dead_code)]
    Entity(Entity),
}

/// Binds a [`DynamicText`](super::dynamictext::DynamicText) to a signal value.
///
/// When attached to an entity with a `DynamicText` component, the
/// [`update_world_signals_binding_system`](crate::systems::signalbinding::update_world_signals_binding_system)
/// will automatically update the text content based on the signal value.
///
/// # Example
///
/// ```ignore
/// // Display score from WorldSignals
/// commands.spawn((
///     DynamicText::new("0", "arcade", 16.0, Color::WHITE),
///     SignalBinding::new("score"),
/// ));
///
/// // Display with custom format
/// commands.spawn((
///     DynamicText::new("", "arcade", 16.0, Color::WHITE),
///     SignalBinding::new("health").with_format("HP: {}"),
/// ));
/// ```
#[derive(Component, Clone, Debug)]
pub struct SignalBinding {
    /// The key of the signal to read from.
    pub signal_key: String,
    /// Optional format string. Use `{}` as a placeholder for the value.
    /// For example: `"Score: {}"` or `"x: {}"`.
    pub format: Option<String>,
    /// Where to read the signal from (world or entity).
    pub source: SignalSource,
}

impl SignalBinding {
    /// Creates a new `SignalBinding` that reads from [`WorldSignals`](crate::resources::worldsignals::WorldSignals).
    ///
    /// # Arguments
    ///
    /// * `signal_key` - The key of the signal to bind to.
    pub fn new(signal_key: impl ToString) -> Self {
        SignalBinding {
            signal_key: signal_key.to_string(),
            format: None,
            source: SignalSource::World,
        }
    }

    /// Sets a format string for the displayed value.
    ///
    /// Use `{}` as a placeholder for the signal value.
    ///
    /// # Example
    ///
    /// ```ignore
    /// SignalBinding::new("score").with_format("Score: {}")
    /// ```
    pub fn with_format(mut self, format: impl ToString) -> Self {
        self.format = Some(format.to_string());
        self
    }

    /// Changes the signal source to read from a specific entity's [`Signals`](super::signals::Signals) component.
    ///
    /// # Arguments
    ///
    /// * `entity` - The entity whose `Signals` component to read from.
    #[allow(dead_code)] // TODO: research use of entities as signal sources
    pub fn with_source_entity(mut self, entity: Entity) -> Self {
        self.source = SignalSource::Entity(entity);
        self
    }
}
