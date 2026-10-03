// 7.7 GUI Widgets
mod gui_widgets {
    use aberredengine::prelude::*;
    use std::sync::Arc;

    fn setup_gui_theme(mut theme_store: ResMut<GuiThemeStore>) {
        let theme = theme_store.themes.entry(Arc::from("default")).or_default();
        theme.panel = GuiNinePatch::new("gui_panel", Rect::new(0.0, 0.0, 64.0, 64.0), 6);
        // hover/pressed/disabled fall back to normal unless set with .with_hover(...) etc.
        theme.button = Some(GuiButtonSkin::new(GuiNinePatch::new(
            "gui_button",
            Rect::new(0.0, 0.0, 32.0, 32.0),
            4,
        )));
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
    hud_theme.panel = GuiNinePatch::new("hud_panel", Rect::new(0.0, 0.0, 48.0, 48.0), 4);
    hud_theme.font = "hud_font".into();

    // Spawn a widget using the "hud" theme
    ctx.commands.spawn((
        GuiWindow::new(200.0, 40.0).with_theme_key("hud"),
        ScreenPosition::new(10.0, 10.0),
        ZIndex(5.0),
    ));
    } // GLUE

    use aberredengine::prelude::*;

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
    use aberredengine::prelude::*;

    fn spawn_smoke(mut commands: Commands) {
        // The template has no MapPosition, so it is never drawn, moved or collided
        let smoke = commands
            .spawn((
                Sprite::new("smoke", 8.0, 8.0).centered(),
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

// 7.9 Attaching Entities (StuckTo)
mod stuck_to {
    use aberredengine::prelude::*;

    fn stick_ball_to_paddle(commands: &mut Commands, ball: Entity, paddle: Entity) {
        commands.entity(ball).insert(
            StuckTo::follow_x_only(paddle)
                .with_offset(Vec2::new(0.0, -12.0))
                .with_stored_velocity(Vec2::new(150.0, -300.0)),
        );
    }

    fn launch_ball(
        mut commands: Commands,
        input: Res<InputState>,
        mut stuck: Query<(Entity, &StuckTo, &mut RigidBody)>,
    ) {
        if !input.action(InputAction::Action1).just_pressed {
            return;
        }
        for (ball, stuck_to, mut rb) in &mut stuck {
            // Removing StuckTo doesn't apply stored_velocity; do it here
            if let Some(velocity) = stuck_to.stored_velocity {
                rb.velocity = velocity;
            }
            commands.entity(ball).remove::<StuckTo>();
        }
    }
}
