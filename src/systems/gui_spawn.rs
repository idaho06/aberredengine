//! Reactive spawn systems for themed GUI widgets.
//!
//! - [`gui_button_spawn_system`] – spawns a `GuiButton`'s `GuiInteractable`
//!   + caption child on `Added<GuiButton>`
//! - [`gui_label_spawn_system`] – spawns a `GuiLabel`'s caption child on
//!   `Added<GuiLabel>`
//! - [`gui_image_spawn_system`] – spawns a `GuiImage`'s `GuiInteractable` +
//!   co-located `Sprite` on `Added<GuiImage>`
//!
//! `GuiButton`/`GuiLabel`/`GuiImage` carry all the data needed to spawn
//! themselves (size, caption/tex_key, callback name) — these systems read
//! that data and insert the secondary components/children, the same
//! pattern [`menu_spawn_system`](super::menu::menu_spawn_system) already
//! uses for `Menu`'s items. Using `insert_if_new` (not `insert`) for the
//! inserted `GuiInteractable`/`Sprite` means a Rust caller that pre-spawned
//! either component in the same bundle as `GuiButton`/`GuiImage` (e.g. to
//! get a Rust fn-pointer click callback via `GuiInteractable::rust(...)`)
//! keeps it — these systems only fill in what's missing.
//!
//! See `docs/gui-system-architecture.md`.

use std::sync::Arc;

use bevy_ecs::prelude::*;
use log::error;
use raylib::prelude::Vector2;

use crate::components::dynamictext::DynamicText;
use crate::components::guibutton::GuiButton;
use crate::components::guiimage::GuiImage;
use crate::components::guiinteractable::GuiInteractable;
use crate::components::guilabel::GuiLabel;
use crate::components::guioffset::GuiOffset;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::guitheme::GuiTheme;

/// Spawns a themed caption child entity for a `GuiButton`/`GuiLabel`,
/// unless `text` is empty (the "captionless widget" signal). `DynamicText`
/// plus `ChildOf(parent)`, `GuiOffset(padding)`, and the same `ZIndex` as
/// the parent — the `Panel`/`Text` `variant_rank` tie-break (see
/// `src/systems/render/mod.rs`) draws text above the parent's background at
/// equal z, so no separate "caption z" is needed. Padding is a fixed
/// constant for v1, not theme-driven: `DynamicText`'s size is only known
/// after a frame (`dynamictext_size_system`), so perfect centering is a
/// future refinement, not a v1 requirement. `font`/`font_size`/`text_color`
/// are resolved from `GuiTheme` (or its defaults when no theme is set),
/// logging the existing "forgot to call engine.set_gui_theme_font" error
/// when `font` is unset. `z_index` defaults to `0.0` when the parent has no
/// `ZIndex` yet.
fn spawn_themed_caption(
    commands: &mut Commands,
    parent: Entity,
    text: &str,
    gui_theme: Option<&GuiTheme>,
    z_index: Option<&ZIndex>,
) {
    if text.is_empty() {
        return;
    }

    const CAPTION_PADDING: Vector2 = Vector2 { x: 8.0, y: 4.0 };

    let default_theme = GuiTheme::default();
    let theme = gui_theme.unwrap_or(&default_theme);
    if theme.font.is_empty() {
        error!(
            "GuiTheme.font is unset — call engine.set_gui_theme_font(...) before spawning \
             GuiButton/GuiLabel captions; the caption entity is still spawned but renders with no visible text"
        );
    }
    commands.spawn((
        DynamicText::new(text, &*theme.font, theme.font_size, theme.text_color),
        ChildOf(parent),
        GuiOffset(CAPTION_PADDING),
        z_index.copied().unwrap_or(ZIndex(0.0)),
    ));
}

/// Spawns entities for newly added [`GuiButton`] components: the
/// co-located `GuiInteractable` (skipped if already present, see module
/// docs) and, unless `caption` is empty, a caption `DynamicText` child.
pub fn gui_button_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &GuiButton, Option<&ZIndex>), Added<GuiButton>>,
    gui_theme: Option<Res<GuiTheme>>,
) {
    for (entity, button, z_index) in &query {
        let mut interactable = GuiInteractable::new(button.size.x, button.size.y);
        if !button.callback_name.is_empty() {
            interactable = interactable.with_on_click_callback(button.callback_name.clone());
        }
        if button.disabled {
            interactable = interactable.with_disabled();
        }
        commands.entity(entity).insert_if_new(interactable);

        spawn_themed_caption(
            &mut commands,
            entity,
            &button.caption,
            gui_theme.as_deref(),
            z_index,
        );
    }
}

/// Spawns the caption `DynamicText` child for newly added [`GuiLabel`]
/// components, unless `caption` is empty.
pub fn gui_label_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &GuiLabel, Option<&ZIndex>), Added<GuiLabel>>,
    gui_theme: Option<Res<GuiTheme>>,
) {
    for (entity, label, z_index) in &query {
        spawn_themed_caption(
            &mut commands,
            entity,
            &label.caption,
            gui_theme.as_deref(),
            z_index,
        );
    }
}

/// Spawns the co-located `GuiInteractable` + `Sprite` for newly added
/// [`GuiImage`] components (skipped per-component if either is already
/// present, see module docs).
pub fn gui_image_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &GuiImage), Added<GuiImage>>,
) {
    for (entity, image) in &query {
        let mut interactable = GuiInteractable::new(image.size.x, image.size.y);
        if !image.callback_name.is_empty() {
            interactable = interactable.with_on_click_callback(image.callback_name.clone());
        }
        commands.entity(entity).insert_if_new((
            interactable,
            Sprite {
                tex_key: Arc::from(image.tex_key.as_str()),
                width: image.size.x,
                height: image.size.y,
                offset: Vector2::new(0.0, 0.0),
                origin: Vector2::new(0.0, 0.0),
                flip_h: false,
                flip_v: false,
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::system::RunSystemOnce;
    use raylib::prelude::Color;

    use super::*;
    use crate::components::guiinteractable::GuiWidgetState;
    use crate::components::screenposition::ScreenPosition;

    fn tick<M>(world: &mut World, system: impl IntoSystem<(), (), M>) {
        world
            .run_system_once(system)
            .expect("system should run without error");
    }

    #[test]
    fn gui_button_spawn_creates_interactable_and_caption() {
        let mut world = World::new();
        world.insert_resource(GuiTheme {
            font: Arc::from("test_font"),
            font_size: 18.0,
            text_color: Color::new(1, 2, 3, 255),
            ..GuiTheme::default()
        });
        let button_entity = world
            .spawn((
                GuiButton {
                callback_name: "on_start_clicked".into(),
                ..GuiButton::new(80.0, 24.0, "Start")
            },
                ScreenPosition::new(10.0, 20.0),
                ZIndex(5.0),
            ))
            .id();

        tick(&mut world, gui_button_spawn_system);

        world
            .get::<GuiButton>(button_entity)
            .expect("button entity should still carry GuiButton");
        let interactable = world
            .get::<GuiInteractable>(button_entity)
            .expect("gui_button_spawn_system should insert GuiInteractable");
        assert_eq!(
            interactable.on_click_callback.as_deref(),
            Some("on_start_clicked")
        );

        let (caption_parent, caption_text, caption_zindex) = world
            .query::<(&ChildOf, &DynamicText, &ZIndex)>()
            .iter(&world)
            .next()
            .expect("caption child entity should be spawned");
        assert_eq!(caption_parent.parent(), button_entity);
        assert_eq!(&*caption_text.text, "Start");
        assert_eq!(caption_zindex.0, 5.0);
        assert_eq!(&*caption_text.font, "test_font");
        assert_eq!(caption_text.font_size, 18.0);
        assert_eq!(caption_text.color, Color::new(1, 2, 3, 255));
    }

    #[test]
    fn gui_button_spawn_with_empty_caption_skips_caption() {
        let mut world = World::new();
        world.spawn((
            GuiButton {
            callback_name: "on_start_clicked".into(),
            ..GuiButton::new(80.0, 24.0, "")
        },
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_button_spawn_system);

        assert_eq!(
            world.query::<&DynamicText>().iter(&world).count(),
            0,
            "no caption child should be spawned for an empty caption"
        );
    }

    #[test]
    fn gui_button_spawn_disabled_applies_to_interactable() {
        let mut world = World::new();
        let button_entity = world
            .spawn((
                GuiButton::new(80.0, 24.0, "").with_disabled(),
                ScreenPosition::new(10.0, 20.0),
                ZIndex(5.0),
            ))
            .id();

        tick(&mut world, gui_button_spawn_system);

        let interactable = world
            .get::<GuiInteractable>(button_entity)
            .expect("GuiInteractable should be inserted");
        assert_eq!(interactable.state, GuiWidgetState::Disabled);
    }

    #[test]
    fn gui_button_spawn_preserves_preexisting_rust_callback_interactable() {
        fn dummy_callback(_entity: Entity, _ctx: &mut crate::systems::GameCtx) {}

        let mut world = World::new();
        let button_entity = world
            .spawn((
                GuiButton::new(80.0, 24.0, "").with_disabled(),
                GuiInteractable::rust(80.0, 24.0, dummy_callback),
                ScreenPosition::new(10.0, 20.0),
                ZIndex(5.0),
            ))
            .id();

        tick(&mut world, gui_button_spawn_system);

        let interactable = world
            .get::<GuiInteractable>(button_entity)
            .expect("GuiInteractable should be present");
        assert!(
            interactable.on_rust_callback.is_some(),
            "insert_if_new must not overwrite a pre-spawned GuiInteractable"
        );
        assert_eq!(
            interactable.state,
            GuiWidgetState::Normal,
            "the pre-spawned GuiInteractable's state must survive, not GuiButton.disabled"
        );
    }

    #[test]
    fn gui_label_spawn_creates_caption() {
        let mut world = World::new();
        world.insert_resource(GuiTheme {
            font: Arc::from("test_font"),
            font_size: 18.0,
            text_color: Color::new(1, 2, 3, 255),
            ..GuiTheme::default()
        });
        let label_entity = world
            .spawn((
                GuiLabel::new(160.0, 24.0, "Hello, GUI!"),
                ScreenPosition::new(10.0, 20.0),
                ZIndex(5.0),
            ))
            .id();

        tick(&mut world, gui_label_spawn_system);

        let (caption_parent, caption_text, caption_zindex) = world
            .query::<(&ChildOf, &DynamicText, &ZIndex)>()
            .iter(&world)
            .next()
            .expect("caption child entity should be spawned");
        assert_eq!(caption_parent.parent(), label_entity);
        assert_eq!(&*caption_text.text, "Hello, GUI!");
        assert_eq!(caption_zindex.0, 5.0);
        assert_eq!(&*caption_text.font, "test_font");
        assert_eq!(caption_text.font_size, 18.0);
        assert_eq!(caption_text.color, Color::new(1, 2, 3, 255));
    }

    #[test]
    fn gui_label_spawn_with_empty_caption_skips_caption() {
        let mut world = World::new();
        world.spawn((
            GuiLabel::new(160.0, 24.0, ""),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        assert_eq!(
            world.query::<&DynamicText>().iter(&world).count(),
            0,
            "no caption child should be spawned for empty caption"
        );
    }

    #[test]
    fn gui_caption_falls_back_to_default_theme_when_no_theme_set() {
        let mut world = World::new();
        world.spawn((
            GuiLabel::new(160.0, 24.0, "Hello, GUI!"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        let caption_text = world
            .query::<&DynamicText>()
            .iter(&world)
            .next()
            .expect("caption should still spawn with default theme values");
        assert_eq!(&*caption_text.font, "");
        assert_eq!(caption_text.font_size, 16.0);
        assert_eq!(caption_text.color, Color::WHITE);
    }

    #[test]
    fn gui_image_spawn_creates_interactable_and_sprite_no_caption() {
        let mut world = World::new();
        world.spawn(GuiImage {
            callback_name: "on_item_clicked".into(),
            ..GuiImage::new(32.0, 32.0, "item_sword")
        });

        tick(&mut world, gui_image_spawn_system);

        let (_entity, interactable, sprite) = world
            .query::<(Entity, &GuiInteractable, &Sprite)>()
            .iter(&world)
            .next()
            .expect("image entity with GuiInteractable + Sprite should be spawned");
        assert!((interactable.size.x - 32.0).abs() < f32::EPSILON);
        assert!((interactable.size.y - 32.0).abs() < f32::EPSILON);
        assert_eq!(
            interactable.on_click_callback.as_deref(),
            Some("on_item_clicked")
        );
        assert_eq!(&*sprite.tex_key, "item_sword");
        assert_eq!(
            world.query::<&ChildOf>().iter(&world).count(),
            0,
            "GuiImage spawns no caption child, unlike GuiButton/GuiLabel"
        );
    }

    #[test]
    fn gui_image_spawn_with_empty_callback_name_skips_callback_wiring() {
        let mut world = World::new();
        world.spawn(GuiImage::new(32.0, 32.0, "item_sword"));

        tick(&mut world, gui_image_spawn_system);

        let interactable = world
            .query::<&GuiInteractable>()
            .iter(&world)
            .next()
            .expect("image entity should be spawned");
        assert!(interactable.on_click_callback.is_none());
    }

    #[test]
    fn gui_image_spawn_preserves_preexisting_sprite_and_interactable() {
        fn dummy_callback(_entity: Entity, _ctx: &mut crate::systems::GameCtx) {}

        let mut world = World::new();
        world.spawn((
            GuiImage::new(32.0, 32.0, "item_sword"),
            GuiInteractable::rust(32.0, 32.0, dummy_callback),
            Sprite {
                tex_key: Arc::from("custom_override"),
                width: 32.0,
                height: 32.0,
                offset: Vector2::new(0.0, 0.0),
                origin: Vector2::new(0.0, 0.0),
                flip_h: false,
                flip_v: false,
            },
        ));

        tick(&mut world, gui_image_spawn_system);

        let (interactable, sprite) = world
            .query::<(&GuiInteractable, &Sprite)>()
            .iter(&world)
            .next()
            .expect("entity should be spawned");
        assert!(
            interactable.on_rust_callback.is_some(),
            "insert_if_new must not overwrite a pre-spawned GuiInteractable"
        );
        assert_eq!(&*sprite.tex_key, "custom_override");
    }
}
