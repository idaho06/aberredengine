use bevy_ecs::prelude::{Component, Entity};

#[derive(Clone, Debug)]
pub enum SignalSource {
    World,
    Entity(Entity),
}

#[derive(Component, Clone, Debug)]
pub struct SignalBinding {
    pub signal_key: String,
    pub format: Option<String>, // e.g., "x: {value}", "y: {value}"
    pub source: SignalSource,   // TODO: World only for now
}

impl SignalBinding {
    pub fn new(signal_key: impl ToString) -> Self {
        SignalBinding {
            signal_key: signal_key.to_string(),
            format: None,
            source: SignalSource::World,
        }
    }
    pub fn with_format(mut self, format: impl ToString) -> Self {
        self.format = Some(format.to_string());
        self
    }
    pub fn with_source_entity(mut self, entity: Entity) -> Self {
        self.source = SignalSource::Entity(entity);
        self
    }
}
