// 7.7 GUI Widgets
mod gui_widgets {
    use aberredengine::bevy_ecs::prelude::ResMut;
    use aberredengine::core::math::{Color, Rect};
    use aberredengine::core::resources::guitheme::{GuiButtonSkin, GuiNinePatch, GuiThemeStore};
    use std::sync::Arc;

    fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
        let theme = theme_store.themes.entry(Arc::from("default")).or_default();
        theme.panel = GuiNinePatch {
            tex_key: "gui_panel".into(),
            source: Rect::new(0.0, 0.0, 64.0, 64.0),
            left: 6,
            top: 6,
            right: 6,
            bottom: 6,
        };
        theme.button = Some(GuiButtonSkin {
            normal: GuiNinePatch {
                tex_key: "gui_button".into(),
                source: Rect::new(0.0, 0.0, 32.0, 32.0),
                left: 4,
                top: 4,
                right: 4,
                bottom: 4,
            },
            ..Default::default() // hover/pressed/disabled fall back to normal if unset
        });
        theme.font = "main_font".into();
        theme.font_size = 16.0;
        theme.text_color = Color::WHITE;

        // Optional: drop shadows. panel_shadow applies to all nine-patch backgrounds;
        // text_shadow is inserted as a Shadow component on spawned caption DynamicText children.
        // theme.panel_shadow = Some(Shadow::default_color(2.0, 2.0));
        // theme.text_shadow = Some(Shadow::default_color(1.0, 1.0));
    }

    fn hud_theme(mut theme_store: ResMut<GuiThemeStore>, ctx: &mut GameCtx) { // GLUE
    let hud_theme = theme_store.themes.entry(Arc::from("hud")).or_default();
    hud_theme.panel = GuiNinePatch { tex_key: "hud_panel".into(), /* source, borders, … */ ..Default::default() };
    hud_theme.font = "hud_font".into();

    // Spawn a widget using the "hud" theme
    ctx.commands.spawn((
        GuiWindow::new(200.0, 40.0).with_theme_key("hud"),
        ScreenPosition::new(10.0, 10.0),
        ZIndex(5.0),
    ));
    } // GLUE

    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::math::Vec2;
    use aberredengine::core::components::guibutton::GuiButton;
    use aberredengine::core::components::guiinteractable::GuiInteractable;
    use aberredengine::core::components::guioffset::GuiOffset;
    use aberredengine::core::components::guiwindow::GuiWindow;
    use aberredengine::core::components::screenposition::ScreenPosition;
    use aberredengine::core::components::zindex::ZIndex;
    use aberredengine::core::systems::GameCtx;

    fn on_start_clicked(_entity: Entity, ctx: &mut GameCtx) {
        ctx.world_signals.set_flag("start_pressed");
    }

    fn spawn_menu_panel(ctx: &mut GameCtx) {
        let panel = ctx
            .commands
            .spawn((
                GuiWindow::new(200.0, 100.0),
                ScreenPosition::new(50.0, 50.0),
                ZIndex(10.0),
            ))
            .id();

        ctx.commands.spawn((
            GuiButton::new(120.0, 32.0, "Start"),
            GuiInteractable::rust(120.0, 32.0, on_start_clicked),
            ChildOf(panel),
            GuiOffset(Vec2::new(40.0, 34.0)),
            ZIndex(10.0),
        ));
    }
}

// 7.8 Particle Emitters
mod particle_emitters {
    use aberredengine::bevy_ecs::prelude::*;
    use aberredengine::core::components::mapposition::MapPosition;
    use aberredengine::core::components::particleemitter::{EmitterShape, ParticleEmitter, TtlSpec};
    use aberredengine::core::components::rigidbody::RigidBody;
    use aberredengine::core::components::sprite::Sprite;
    use aberredengine::core::components::zindex::ZIndex;
    use aberredengine::core::math::Vec2;
    use std::sync::Arc;

    fn spawn_smoke(mut commands: Commands) {
        // The template has no MapPosition, so it is never drawn, moved or collided
        let smoke = commands
            .spawn((
                Sprite {
                    tex_key: Arc::from("smoke"),
                    width: 8.0,
                    height: 8.0,
                    offset: Vec2::ZERO,
                    origin: Vec2::new(4.0, 4.0),
                    flip_h: false,
                    flip_v: false,
                },
                ZIndex(5.0),
                RigidBody::with_physics(2.0, None), // every particle keeps this friction
            ))
            .id();

        commands.spawn((
            MapPosition::new(100.0, 100.0),
            ParticleEmitter {
                templates: vec![smoke],
                shape: EmitterShape::Rect { width: 16.0, height: 4.0 },
                particles_per_emission: 3,
                emissions_per_second: 10.0,
                emissions_remaining: 100,
                initial_emissions_remaining: 100,
                arc_degrees: (-30.0, 30.0), // 0° is up, angles grow clockwise
                speed_range: (50.0, 100.0),
                ttl: TtlSpec::Range { min: 0.5, max: 1.0 },
                ..Default::default()
            },
        ));
    }
}
