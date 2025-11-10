use crate::components::group::Group;
use crate::components::mapposition::MapPosition;
use crate::components::menu::Menu;
use crate::components::screenposition::ScreenPosition;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::events::input::{InputAction, InputEvent};
use crate::resources::fontstore::FontStore;
use crate::resources::input::InputState;
use crate::resources::texturestore::TextureStore;
use crate::{components::dynamictext::DynamicText, game::load_texture_from_text};
use bevy_ecs::prelude::*;
use raylib::prelude::{Color, Font, Vector2};
use raylib::texture;

pub fn menu_spawn_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Menu), Added<Menu>>,
    font_store: NonSend<FontStore>,
    mut texture_store: ResMut<TextureStore>,
    mut rl: NonSendMut<raylib::RaylibHandle>,
    th: NonSend<raylib::RaylibThread>,
) {
    for (entity, mut menu) in query.iter_mut() {
        // Cache immutable data before mutable iteration to satisfy borrow rules
        let font_string = menu.font.clone();
        let font_size = menu.font_size;
        let normal_color = menu.normal_color;
        let use_screen_space = menu.use_screen_space;

        // Spawn DynamicText or Sprite for each menu item if needed
        for item in menu.items.iter_mut() {
            let mut ecmd = commands.spawn_empty();
            if item.dynamic_text {
                // Dynamic text will be updated each frame
                ecmd.insert(DynamicText::new(
                    &item.label,
                    font_string.clone(),
                    font_size,
                    normal_color,
                ));
            } else {
                // Static text sprite
                let font_handle = font_store.get(&font_string).expect(&format!(
                    "menu_spawn_system: Font {} not found in FontStore",
                    font_string
                ));
                let texture_handle = load_texture_from_text(
                    &mut rl,
                    &th,
                    font_handle,
                    &item.label,
                    font_size,
                    1.0,
                    normal_color,
                )
                .expect("Failed to create texture from text");
                let width = texture_handle.width as f32;
                let height = texture_handle.height as f32;
                texture_store.insert(&format!("menu_{}", item.id), texture_handle);
                ecmd.insert(Sprite {
                    tex_key: format!("menu_{}", item.id),
                    width,
                    height,
                    offset: Vector2 { x: 0.0, y: 0.0 },
                    origin: Vector2 { x: 0.0, y: 0.0 },
                    flip_h: false,
                    flip_v: false,
                });
            }

            if use_screen_space {
                ecmd.insert(ScreenPosition { pos: item.position });
            } else {
                ecmd.insert(MapPosition { pos: item.position });
                ecmd.insert(ZIndex(23));
            }
            let text_entity = ecmd.id();
            ecmd.insert(Group::new(&format!("menu_{}", entity.to_string())));
            item.entity = Some(text_entity);
        }

        // Spawn cursor entity if needed
        if let Some(cursor_entity) = menu.cursor_entity {
            let cursor_position = menu.items[menu.selected_index].position; // make sure sprite has correct origin
            if use_screen_space {
                commands.entity(cursor_entity).insert(ScreenPosition {
                    pos: cursor_position,
                });
            } else {
                commands.entity(cursor_entity).insert(MapPosition {
                    pos: cursor_position,
                });
                commands.entity(cursor_entity).insert(ZIndex(23));
            }
        }
    }
}

pub fn menu_controller_observer(
    trigger: On<InputEvent>,
    mut query: Query<(&mut Menu,)>,
    mut commands: Commands,
) {
    for (mut menu,) in query.iter_mut() {
        if !menu.active {
            continue;
        }

        if !trigger.event().pressed {
            continue; // Only handle key press, not release
        }

        let mut changed_selection = false;
        match trigger.event().action {
            InputAction::SecondaryDirectionUp => {
                if menu.selected_index == 0 {
                    menu.selected_index = menu.items.len() - 1;
                } else {
                    menu.selected_index -= 1;
                }
                changed_selection = true;
            }
            InputAction::SecondaryDirectionDown => {
                menu.selected_index = (menu.selected_index + 1) % menu.items.len();
                changed_selection = true;
            }
            _ => {}
        }

        // Update cursor position if applicable
        if changed_selection {
            if let Some(cursor_entity) = menu.cursor_entity {
                let cursor_position = menu.items[menu.selected_index].position;
                if menu.use_screen_space {
                    commands.entity(cursor_entity).insert(ScreenPosition {
                        pos: cursor_position,
                    });
                } else {
                    commands.entity(cursor_entity).insert(MapPosition {
                        pos: cursor_position,
                    });
                }
            }
        }
    }
}
