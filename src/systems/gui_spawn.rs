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
use log::{error, warn};
use raylib::prelude::Vector2;

use crate::components::dynamictext::DynamicText;
use crate::components::guibutton::GuiButton;
use crate::components::guiimage::GuiImage;
use crate::components::guiinteractable::GuiInteractable;
use crate::components::guilabel::GuiLabel;
use crate::components::guioffset::GuiOffset;
use crate::components::signalbinding::SignalBinding;
use crate::components::sprite::Sprite;
use crate::components::zindex::ZIndex;
use crate::resources::guitheme::{GuiTheme, GuiThemeStore, GuiThemeWarnCache};

/// Spawns a themed caption child entity for a `GuiButton`/`GuiLabel`,
/// unless `text` is empty (the "captionless widget" signal). `DynamicText`
/// plus `ChildOf(parent)`, `GuiOffset(padding)`, and the same `ZIndex` as
/// the parent — the `Panel`/`Text` `variant_rank` tie-break (see
/// `src/systems/render/mod.rs`) draws text above the parent's background at
/// equal z, so no separate "caption z" is needed. Padding is a fixed
/// constant for v1, not theme-driven: `DynamicText`'s size is only known
/// after a frame (`dynamictext_size_system`), so perfect centering is a
/// future refinement, not a v1 requirement. `font`/`font_size`/`text_color`
/// are resolved from the named theme (`theme_key`) in `GuiThemeStore` (or
/// built-in defaults if the key isn't registered, with a one-time warn via
/// `gui_theme_warn_cache`), logging the existing "forgot to call
/// engine.set_gui_theme_font" error when `font` is unset. `z_index`
/// defaults to `0.0` when the parent has no `ZIndex` yet. `signal_binding`
/// (currently only ever passed by `gui_label_spawn_system`, never by
/// `gui_button_spawn_system`), if `Some`, attaches a `SignalBinding` to the
/// caption so it auto-updates from `WorldSignals`.
#[allow(clippy::too_many_arguments)]
fn spawn_themed_caption(
    commands: &mut Commands,
    parent: Entity,
    text: &str,
    theme_key: &str,
    gui_theme_store: &GuiThemeStore,
    gui_theme_warn_cache: &mut GuiThemeWarnCache,
    z_index: Option<&ZIndex>,
    signal_binding: Option<&(String, Option<String>)>,
) {
    if text.is_empty() {
        return;
    }

    const CAPTION_PADDING: Vector2 = Vector2 { x: 8.0, y: 4.0 };

    let default_theme;
    let theme = match gui_theme_store.get(theme_key) {
        Some(theme) => theme,
        None => {
            if gui_theme_warn_cache.warn_once(theme_key) {
                warn!(
                    "Caption theme_key '{}' not registered in GuiThemeStore — using built-in defaults",
                    theme_key
                );
            }
            default_theme = GuiTheme::default();
            &default_theme
        }
    };
    if theme.font.is_empty() {
        error!(
            "GuiTheme '{}'.font is unset — call engine.set_gui_theme_font(\"{}\", ...) before spawning \
             GuiButton/GuiLabel captions; the caption entity is still spawned but renders with no visible text",
            theme_key, theme_key
        );
    }
    let mut caption = commands.spawn((
        DynamicText::new(text, &*theme.font, theme.font_size, theme.text_color),
        ChildOf(parent),
        GuiOffset(CAPTION_PADDING),
        z_index.copied().unwrap_or(ZIndex(0.0)),
    ));
    if let Some((key, format)) = signal_binding {
        let mut binding = SignalBinding::new(key);
        if let Some(fmt) = format {
            binding = binding.with_format(fmt);
        }
        caption.insert(binding);
    }
}

/// Spawns entities for newly added [`GuiButton`] components: the
/// co-located `GuiInteractable` (skipped if already present, see module
/// docs) and, unless `caption` is empty, a caption `DynamicText` child.
pub fn gui_button_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &GuiButton, Option<&ZIndex>), Added<GuiButton>>,
    gui_theme_store: Res<GuiThemeStore>,
    mut gui_theme_warn_cache: ResMut<GuiThemeWarnCache>,
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
            &button.theme_key,
            &gui_theme_store,
            &mut gui_theme_warn_cache,
            z_index,
            None,
        );
    }
}

/// Spawns the caption `DynamicText` child for newly added [`GuiLabel`]
/// components, unless `caption` is empty. If `label.signal_binding` is set,
/// the caption also gets a `SignalBinding`, so
/// `update_world_signals_binding_system` keeps it in sync with
/// `WorldSignals` -- `caption` remains the placeholder shown until the
/// signal key first resolves.
pub fn gui_label_spawn_system(
    mut commands: Commands,
    query: Query<(Entity, &GuiLabel, Option<&ZIndex>), Added<GuiLabel>>,
    gui_theme_store: Res<GuiThemeStore>,
    mut gui_theme_warn_cache: ResMut<GuiThemeWarnCache>,
) {
    for (entity, label, z_index) in &query {
        spawn_themed_caption(
            &mut commands,
            entity,
            &label.caption,
            &label.theme_key,
            &gui_theme_store,
            &mut gui_theme_warn_cache,
            z_index,
            label.signal_binding.as_ref(),
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
    use crate::components::guiwindow::GuiWindow;
    use crate::components::screenposition::ScreenPosition;

    fn tick<M>(world: &mut World, system: impl IntoSystem<(), (), M>) {
        world
            .run_system_once(system)
            .expect("system should run without error");
    }

    fn store_with_default(theme: GuiTheme) -> GuiThemeStore {
        GuiThemeStore {
            themes: std::iter::once((Arc::from("default"), theme)).collect(),
        }
    }

    fn insert_empty_theme_store(world: &mut World) {
        world.insert_resource(GuiThemeStore::default());
        world.insert_resource(GuiThemeWarnCache::default());
    }

    #[test]
    fn gui_button_spawn_creates_interactable_and_caption() {
        let mut world = World::new();
        world.insert_resource(store_with_default(GuiTheme {
            font: Arc::from("test_font"),
            font_size: 18.0,
            text_color: Color::new(1, 2, 3, 255),
            ..GuiTheme::default()
        }));
        world.insert_resource(GuiThemeWarnCache::default());
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
        insert_empty_theme_store(&mut world);
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
        insert_empty_theme_store(&mut world);
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
        insert_empty_theme_store(&mut world);
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
        world.insert_resource(store_with_default(GuiTheme {
            font: Arc::from("test_font"),
            font_size: 18.0,
            text_color: Color::new(1, 2, 3, 255),
            ..GuiTheme::default()
        }));
        world.insert_resource(GuiThemeWarnCache::default());
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
        insert_empty_theme_store(&mut world);
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
    fn gui_label_spawn_with_signal_binding_attaches_binding_to_caption() {
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.insert_resource(store_with_default(GuiTheme::default()));
        world.spawn((
            GuiLabel::new(160.0, 24.0, "0").with_signal_binding("score"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        let binding = world
            .query::<&SignalBinding>()
            .iter(&world)
            .next()
            .expect("caption child should have a SignalBinding when GuiLabel.signal_binding is set");
        assert_eq!(binding.signal_key, "score");
        assert_eq!(binding.format, None);
    }

    #[test]
    fn gui_label_spawn_signal_binding_format_is_attached() {
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.insert_resource(store_with_default(GuiTheme::default()));
        world.spawn((
            GuiLabel::new(160.0, 24.0, "0")
                .with_signal_binding("hp")
                .with_signal_binding_format("HP: {}"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        let binding = world
            .query::<&SignalBinding>()
            .iter(&world)
            .next()
            .expect("caption child should have a SignalBinding");
        assert_eq!(binding.signal_key, "hp");
        assert_eq!(binding.format.as_deref(), Some("HP: {}"));
    }

    #[test]
    fn gui_label_spawn_without_signal_binding_has_no_binding() {
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.insert_resource(store_with_default(GuiTheme::default()));
        world.spawn((
            GuiLabel::new(160.0, 24.0, "Hello, GUI!"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        assert_eq!(
            world.query::<&SignalBinding>().iter(&world).count(),
            0,
            "no SignalBinding should be attached when GuiLabel.signal_binding is unset"
        );
    }

    #[test]
    fn gui_button_caption_never_gets_a_signal_binding() {
        // gui_button_spawn_system always passes None for signal_binding --
        // SignalBinding wiring is GuiLabel-only per the design doc's
        // Roadmap item #3.
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.insert_resource(store_with_default(GuiTheme::default()));
        world.spawn((
            GuiButton::new(80.0, 24.0, "Start"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_button_spawn_system);

        assert_eq!(world.query::<&SignalBinding>().iter(&world).count(), 0);
    }

    #[test]
    fn gui_label_signal_binding_placeholder_then_live_update() {
        use crate::resources::worldsignals::WorldSignals;
        use crate::systems::signalbinding::update_world_signals_binding_system;

        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.insert_resource(store_with_default(GuiTheme::default()));
        world.insert_resource(WorldSignals::default());
        world.spawn((
            GuiLabel::new(160.0, 24.0, "0").with_signal_binding("score"),
            ScreenPosition::new(10.0, 20.0),
            ZIndex(5.0),
        ));

        tick(&mut world, gui_label_spawn_system);
        tick(&mut world, update_world_signals_binding_system);

        let caption_text = world
            .query::<&DynamicText>()
            .iter(&world)
            .next()
            .expect("caption should be spawned")
            .text
            .clone();
        assert_eq!(
            &*caption_text, "0",
            "caption keeps the placeholder while the signal key is unset"
        );

        world.resource_mut::<WorldSignals>().set_integer("score", 42);
        tick(&mut world, update_world_signals_binding_system);

        let caption_text = world
            .query::<&DynamicText>()
            .iter(&world)
            .next()
            .expect("caption should still exist")
            .text
            .clone();
        assert_eq!(&*caption_text, "42");
    }

    #[test]
    fn gui_caption_falls_back_to_default_theme_when_no_theme_set() {
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
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
        assert!(
            !world
                .resource_mut::<GuiThemeWarnCache>()
                .warn_once("default"),
            "missing theme_key should be recorded after the fallback"
        );
    }

    #[test]
    fn gui_button_spawn_uses_theme_key_to_resolve_caption_font() {
        let mut world = World::new();
        world.insert_resource(GuiThemeStore {
            themes: [
                (
                    Arc::from("theme_a"),
                    GuiTheme {
                        font: Arc::from("font_a"),
                        ..GuiTheme::default()
                    },
                ),
                (
                    Arc::from("theme_b"),
                    GuiTheme {
                        font: Arc::from("font_b"),
                        ..GuiTheme::default()
                    },
                ),
            ]
            .into_iter()
            .collect(),
        });
        world.insert_resource(GuiThemeWarnCache::default());
        world.spawn((
            GuiButton::new(80.0, 24.0, "A").with_theme_key("theme_a"),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));
        world.spawn((
            GuiButton::new(80.0, 24.0, "B").with_theme_key("theme_b"),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));

        tick(&mut world, gui_button_spawn_system);

        let mut fonts: Vec<String> = world
            .query::<&DynamicText>()
            .iter(&world)
            .map(|t| t.font.to_string())
            .collect();
        fonts.sort();
        assert_eq!(fonts, vec!["font_a".to_string(), "font_b".to_string()]);
    }

    #[test]
    fn gui_button_does_not_inherit_parent_window_theme_key() {
        // Locks in the Option-A decision (flat, explicit theme_key, no
        // hierarchy inheritance) against regression: a GuiButton parented
        // under a differently-themed GuiWindow must keep its own default
        // theme_key, not silently pick up the window's.
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        let window = world
            .spawn(GuiWindow::new(200.0, 150.0).with_theme_key("window_theme"))
            .id();
        world.spawn((
            GuiButton::new(80.0, 24.0, ""),
            ChildOf(window),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));

        tick(&mut world, gui_button_spawn_system);

        let button = world
            .query::<&GuiButton>()
            .iter(&world)
            .next()
            .expect("button should exist");
        assert_eq!(&*button.theme_key, "default");
    }

    #[test]
    fn gui_label_spawn_with_unregistered_theme_key_warns_and_uses_default_font() {
        let mut world = World::new();
        insert_empty_theme_store(&mut world);
        world.spawn((
            GuiLabel::new(160.0, 24.0, "Hello").with_theme_key("missing"),
            ScreenPosition::new(0.0, 0.0),
            ZIndex(0.0),
        ));

        tick(&mut world, gui_label_spawn_system);

        let caption_text = world
            .query::<&DynamicText>()
            .iter(&world)
            .next()
            .expect("caption should still spawn with default values");
        assert_eq!(&*caption_text.font, "");
        assert!(
            !world.resource_mut::<GuiThemeWarnCache>().warn_once("missing"),
            "the missing key should already be recorded by spawn_themed_caption's fallback"
        );
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
