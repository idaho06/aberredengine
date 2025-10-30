use bevy_ecs::prelude::Component;

#[derive(Component, Clone, Debug)]
/// Dynamic text component for rendering variable strings in the world or screen.
pub struct DynamicText {
    /// The text content to render.
    pub content: String,
    /// Font type
    pub font: String,
    /// Font size in world units.
    pub font_size: f32,
    /// Color of the text.
    pub color: raylib::prelude::Color,
}

impl DynamicText {
    /// Creates a new DynamicText component.
    pub fn new(
        content: impl Into<String>,
        font: impl Into<String>,
        font_size: f32,
        color: raylib::prelude::Color,
    ) -> Self {
        Self {
            content: content.into(),
            font: font.into(),
            font_size,
            color,
        }
    }
    /// Updates the text content.
    pub fn set_content(&mut self, new_content: impl Into<String>) {
        self.content = new_content.into();
    }
}
