use bevy_ecs::prelude::Resource;
use bevy_ecs::system::SystemId;
use rustc_hash::FxHashMap;

#[derive(Resource)]
pub struct SystemsStore {
    pub map: FxHashMap<String, SystemId>,
}

impl SystemsStore {
    pub fn new() -> Self {
        SystemsStore {
            map: FxHashMap::default(),
        }
    }

    pub fn insert(&mut self, name: impl Into<String>, id: SystemId) {
        self.map.insert(name.into(), id);
    }

    pub fn get(&self, name: impl Into<String>) -> Option<&SystemId> {
        self.map.get(&name.into())
    }
}
