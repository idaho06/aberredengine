use bevy_ecs::prelude::Resource;
use raylib::prelude::Font;
use rustc_hash::FxHashMap;

// NonSend resource: insert with insert_non_send_resource and access via NonSend/NonSendMut
pub struct FontStore {
    fonts: FxHashMap<String, Font>,
}

impl FontStore {
    pub fn new() -> Self {
        Self {
            fonts: FxHashMap::default(),
        }
    }

    pub fn add(&mut self, id: impl Into<String>, font: Font) {
        self.fonts.insert(id.into(), font);
    }

    pub fn get(&self, id: &str) -> Option<&Font> {
        self.fonts.get(id)
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
    }

    pub fn len(&self) -> usize {
        self.fonts.len()
    }
}
