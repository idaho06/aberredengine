use bevy_ecs::prelude::{Component, Entity};
use raylib::prelude::{Color, Vector2};

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub id: String,
    pub label: String,
    pub position: Vector2,
    pub dynamic_text: bool,
    pub enabled: bool,
    pub entity: Option<Entity>, // If not dynamic_text, the entity holding the text sprite
}

#[derive(Component, Clone, Debug)]
pub struct Menu {
    pub items: Vec<MenuItem>,
    pub selected_index: usize,
    pub font: String,
    pub font_size: f32,
    pub item_spacing: f32,
    pub normal_color: Color,
    pub selected_color: Color,
    pub cursor_entity: Option<Entity>, // Sprite of a pointer or highlight
    pub origin: Vector2,
    pub use_screen_space: bool,
}

impl Menu {
    pub fn new(
        labels: &[(&str, &str)], // (id, label)
        origin: Vector2,
        font: impl Into<String>,
        font_size: f32,
        item_spacing: f32,
        use_screen_space: bool,
    ) -> Self {
        let options = labels
            .iter()
            .enumerate()
            .map(|(i, (id, label))| MenuItem {
                id: id.to_string(),
                label: label.to_string(),
                position: Vector2 {
                    x: origin.x,
                    y: origin.y + i as f32 * item_spacing,
                },
                dynamic_text: true,
                enabled: true,
                entity: None,
            })
            .collect();
        Self {
            items: options,
            selected_index: 0,
            font: font.into(),
            font_size,
            item_spacing,
            normal_color: Color::WHITE,
            selected_color: Color::YELLOW,
            cursor_entity: None,
            origin,
            use_screen_space,
        }
    }
    pub fn with_cursor(mut self, cursor_entity: Entity) -> Self {
        self.cursor_entity = Some(cursor_entity);
        self
    }
    pub fn with_colors(mut self, normal: Color, selected: Color) -> Self {
        self.normal_color = normal;
        self.selected_color = selected;
        self
    }
    pub fn with_dynamic_text(mut self, dynamic: bool) -> Self {
        for item in &mut self.items {
            item.dynamic_text = dynamic;
        }
        self
    }
}
